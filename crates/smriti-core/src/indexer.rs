//! Background indexing: metadata, fingerprint, thumbnail and faces for every
//! queued asset. Work survives restarts because the queue lives in the `jobs`
//! table.
//!
//! Two job stages: `index` (everything, for new or changed files) and `faces`
//! (face analysis only, for photos indexed before the face models were there).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bytes::Bytes;
use crossbeam_channel::bounded;
use image::RgbImage;
use nom_exif::MediaParser;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::faces::{self, AnalyzedFace, FaceEngine};
use crate::media::{self, Kind};
use crate::metadata::{self, Meta};
use crate::thumbs::{self, ThumbError};
use crate::{Id, Library, Result};

const BATCH: usize = 64;
/// Photos buffered ahead of the decoders (about 16 x 6 MB).
const READ_AHEAD: usize = 16;
const MAX_BUFFERED: i64 = 64 * 1024 * 1024;
const MAX_ATTEMPTS: i64 = 3;
const FACE_CROP_QUALITY: f32 = 82.0;

#[derive(Debug, Clone, Copy, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    /// Items in this run (done + failed + still queued).
    pub total: u32,
    pub done: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Index,
    Faces,
}

struct Job {
    id: i64,
    asset: Id,
    stage: Stage,
    path: PathBuf,
    kind: Kind,
    mime: String,
    size: i64,
    mtime: i64,
    orientation: Option<u16>,
}

/// Face models are loaded per worker on first use; a failure is remembered so
/// it is logged once rather than per photo.
enum Engine {
    NotLoaded,
    Ready(Box<FaceEngine>),
    Unavailable,
}

impl Engine {
    fn get(&mut self, models: Option<&Path>) -> Option<&mut FaceEngine> {
        if let Engine::NotLoaded = self {
            *self = match models.map(FaceEngine::load) {
                Some(Ok(e)) => Engine::Ready(Box::new(e)),
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "face models could not be loaded");
                    Engine::Unavailable
                }
                None => Engine::Unavailable,
            };
        }
        match self {
            Engine::Ready(e) => Some(e),
            _ => None,
        }
    }
}

impl Library {
    /// Processes every queued job with `workers` threads, calling `progress`
    /// after each item. Returns when the queue is empty or `cancel` is set.
    pub fn run_index(
        &self,
        workers: usize,
        cancel: &AtomicBool,
        progress: &(dyn Fn(IndexProgress) + Sync),
    ) -> Result<IndexProgress> {
        self.run_index_upto(workers, u32::MAX, cancel, progress)
    }

    /// Like [`Library::run_index`], stopping after about `max_jobs` items so
    /// the caller can do other work (such as recognising people) in between.
    pub fn run_index_upto(
        &self,
        workers: usize,
        max_jobs: u32,
        cancel: &AtomicBool,
        progress: &(dyn Fn(IndexProgress) + Sync),
    ) -> Result<IndexProgress> {
        let models = self.models_dir();
        let with_faces = models.is_some();
        let (queued, run_started): (u32, String) = self.conn().query_row(
            "SELECT count(*), strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM jobs
             WHERE state = 'queued' AND (stage = 'index' OR (?1 AND stage = 'faces'))",
            [with_faces],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let total = queued.min(max_jobs);
        if total == 0 {
            return Ok(IndexProgress::default());
        }
        let done = AtomicU32::new(0);
        let failed = AtomicU32::new(0);
        let report =
            || IndexProgress { total, done: done.load(Ordering::Relaxed), failed: failed.load(Ordering::Relaxed) };
        tracing::info!(total, workers, with_faces, "indexing started");
        let started = std::time::Instant::now();

        // One thread reads files sequentially; workers decode from memory.
        // On spinning disks parallel reads thrash the head (measured: 7 readers
        // ran 3.5x slower than 1 reader feeding 7 decoders).
        let (tx, rx) = bounded::<(Job, Option<Bytes>)>(READ_AHEAD);
        let (done_ref, failed_ref, report_ref) = (&done, &failed, &report);
        std::thread::scope(|s| -> Result<()> {
            for _ in 0..workers.max(1) {
                let rx = rx.clone();
                let models = models.as_deref();
                s.spawn(move || {
                    let mut parser = MediaParser::new();
                    let mut engine = Engine::NotLoaded;
                    for (job, data) in rx {
                        let ok = match job.stage {
                            Stage::Index => self.process(&mut parser, &mut engine, models, &job, data),
                            Stage::Faces => self.process_faces(&mut engine, models, &job, data),
                        };
                        (if ok { done_ref } else { failed_ref }).fetch_add(1, Ordering::Relaxed);
                        progress(report_ref());
                    }
                });
            }
            drop(rx);
            // Feed workers until the queue is drained, the limit is hit or we're cancelled.
            let mut fed = 0u32;
            'feed: while !cancel.load(Ordering::Relaxed) && fed < total {
                let batch = self.claim(BATCH.min((total - fed) as usize), &run_started, with_faces)?;
                if batch.is_empty() {
                    break;
                }
                for job in batch {
                    fed += 1;
                    // Videos are read by FFmpeg itself; huge files aren't worth buffering.
                    let data = (job.kind == Kind::Photo && job.size < MAX_BUFFERED)
                        .then(|| std::fs::read(&job.path).ok().map(Bytes::from))
                        .flatten();
                    if tx.send((job, data)).is_err() || cancel.load(Ordering::Relaxed) {
                        break 'feed;
                    }
                }
            }
            drop(tx);
            Ok(())
        })?;

        // Anything claimed but not processed (cancel) goes back in the queue.
        self.conn().execute("UPDATE jobs SET state = 'queued' WHERE state = 'running'", [])?;
        let r = report();
        tracing::info!(?r, secs = started.elapsed().as_secs(), "indexing finished");
        Ok(r)
    }

    /// Claims up to `n` jobs queued before this run started, so items that
    /// fail during the run wait for the next run instead of looping.
    fn claim(&self, n: usize, run_started: &str, with_faces: bool) -> Result<Vec<Job>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "UPDATE jobs SET state = 'running', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id IN (SELECT id FROM jobs
                          WHERE state = 'queued' AND updated_at <= ?2 AND (stage = 'index' OR (?3 AND stage = 'faces'))
                          ORDER BY priority, id LIMIT ?1)
             RETURNING id, asset_id, stage",
        )?;
        let claimed: Vec<(i64, Id, String)> = stmt
            .query_map(params![n as i64, run_started, with_faces], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut q = conn.prepare_cached(
            "SELECT s.path, a.rel_path, a.kind, a.mime, a.size, a.mtime, a.orientation
             FROM assets a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
        )?;
        let mut jobs = Vec::with_capacity(claimed.len());
        for (id, asset, stage) in claimed {
            let row = q
                .query_row([asset], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                })
                .optional()?;
            match row {
                Some((root, rel, kind, mime, size, mtime, orientation)) => jobs.push(Job {
                    id,
                    asset,
                    stage: if stage == "faces" { Stage::Faces } else { Stage::Index },
                    path: Path::new(&root).join(rel),
                    kind: Kind::parse(&kind).unwrap_or(Kind::Photo),
                    mime: mime.unwrap_or_default(),
                    size,
                    mtime,
                    orientation,
                }),
                None => {
                    conn.execute("DELETE FROM jobs WHERE id = ?1", [id])?;
                }
            }
        }
        Ok(jobs)
    }

    /// Returns false when the item failed (it is retried up to MAX_ATTEMPTS).
    fn process(
        &self,
        parser: &mut MediaParser,
        engine: &mut Engine,
        models: Option<&Path>,
        job: &Job,
        data: Option<Bytes>,
    ) -> bool {
        let meta = metadata::read(parser, &job.path, job.kind, job.mtime, data.as_ref());
        let fingerprint = match &data {
            Some(d) => Some(fingerprint_bytes(d, &meta)),
            None => fingerprint(&job.path, job.size, &meta).ok(),
        };

        let dest = thumbs::thumb_path(&self.thumbs_dir(), job.asset);
        let mut found_faces = None;
        let decodable = job.kind == Kind::Photo && media::decodable_photo(&job.mime);
        let thumb = match job.kind {
            Kind::Photo if decodable => {
                // One decode serves both the thumbnail and face analysis.
                let engine = engine.get(models);
                let min_short = if engine.is_some() { faces::DECODE_SHORT_SIDE } else { thumbs::THUMB_HEIGHT };
                decode_upright(&job.path, data.as_deref(), min_short, meta.orientation).and_then(|img| {
                    if let Some(e) = engine {
                        found_faces = Some(e.analyze(&img));
                    }
                    thumbs::write_oriented(img, &dest)
                })
            }
            Kind::Video => thumbs::make_video(self.ffmpeg(), &job.path, meta.duration_ms, &dest),
            Kind::Photo => Err(ThumbError::Decode("format not supported yet".into())),
        };
        let (thumb_state, dims, error) = match &thumb {
            Ok(dims) => ("ok", Some(*dims), None),
            Err(ThumbError::Decode(m)) if m.contains("not supported") => ("unsupported", None, None),
            Err(ThumbError::NoFfmpeg) => ("unsupported", None, None),
            Err(e) => ("failed", None, Some(e.to_string())),
        };

        let mut result = self.save(job, &meta, fingerprint, thumb_state, dims);
        if result.is_ok() && decodable && thumb.is_ok() {
            result = match found_faces {
                Some(Ok(found)) => self.save_faces(job.asset, found),
                Some(Err(e)) => {
                    tracing::debug!(path = %job.path.display(), error = %e, "face analysis failed; will retry");
                    self.queue_faces(job)
                }
                // No models yet: analyse once they arrive.
                None => self.queue_faces(job),
            };
        }
        if result.is_ok() {
            result = self.finish_job(job, error.as_deref()).map(|_| ());
        }
        if let Err(e) = &result {
            tracing::warn!(path = %job.path.display(), error = %e, "could not save index result");
        }
        error.is_none() && result.is_ok()
    }

    /// Face analysis alone, for photos indexed before the models were available.
    fn process_faces(&self, engine: &mut Engine, models: Option<&Path>, job: &Job, data: Option<Bytes>) -> bool {
        let outcome = match engine.get(models) {
            None => Err("face models are not available".to_string()),
            Some(e) => decode_upright(&job.path, data.as_deref(), faces::DECODE_SHORT_SIDE, job.orientation)
                .map_err(|e| e.to_string())
                .and_then(|img| e.analyze(&img).map_err(|e| e.to_string())),
        };
        let result = match outcome {
            Ok(found) => self.save_faces(job.asset, found).and_then(|_| self.finish_job(job, None)),
            Err(e) => self.finish_job(job, Some(&e)),
        };
        if let Err(e) = &result {
            tracing::warn!(path = %job.path.display(), error = %e, "could not save faces");
        }
        matches!(result, Ok(true))
    }

    fn save(
        &self,
        job: &Job,
        m: &Meta,
        fingerprint: Option<String>,
        thumb_state: &str,
        thumb_dims: Option<(u32, u32)>,
    ) -> Result<()> {
        // Prefer header dimensions; fall back to the thumbnail's aspect ratio.
        let (w, h) = match (m.width, m.height, thumb_dims) {
            (Some(w), Some(h), _) => (Some(w), Some(h)),
            (_, _, Some((w, h))) => (Some(w), Some(h)),
            _ => (None, None),
        };
        self.conn().execute(
            "UPDATE assets SET taken_at = ?2, taken_at_source = ?3, tz_offset_min = ?4, width = ?5, height = ?6,
                 orientation = ?7, lat = ?8, lon = ?9, camera = ?10, duration_ms = ?11, fingerprint = ?12,
                 thumb_state = ?13, indexed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![
                job.asset,
                m.taken_at.map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
                m.date_source.map(|s| s.as_str()),
                m.tz_offset_min,
                w,
                h,
                m.orientation,
                m.lat,
                m.lon,
                m.camera,
                m.duration_ms.map(|d| d as i64),
                fingerprint,
                thumb_state,
            ],
        )?;
        Ok(())
    }

    /// Removes a finished job, or records the failure for a later retry.
    /// Returns true when the job succeeded.
    fn finish_job(&self, job: &Job, error: Option<&str>) -> Result<bool> {
        let conn = self.conn();
        match error {
            None => {
                conn.execute("DELETE FROM jobs WHERE id = ?1", [job.id])?;
                Ok(true)
            }
            Some(err) => {
                tracing::debug!(path = %job.path.display(), error = err, "index item failed");
                conn.execute(
                    "UPDATE jobs SET attempts = attempts + 1, last_error = ?2,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         state = CASE WHEN attempts + 1 >= ?3 THEN 'failed' ELSE 'queued' END
                     WHERE id = ?1",
                    params![job.id, err, MAX_ATTEMPTS],
                )?;
                Ok(false)
            }
        }
    }

    fn queue_faces(&self, job: &Job) -> Result<()> {
        self.conn().execute(
            "INSERT INTO jobs (asset_id, stage, priority) VALUES (?1, 'faces', ?2)
             ON CONFLICT (asset_id, stage) DO UPDATE SET state = 'queued', attempts = 0, last_error = NULL",
            params![job.asset, -job.mtime],
        )?;
        Ok(())
    }

    /// Replaces the asset's faces with a fresh analysis and writes their crops.
    fn save_faces(&self, asset: Id, found: Vec<AnalyzedFace>) -> Result<()> {
        let mut new_ids = Vec::with_capacity(found.len());
        let old_ids: Vec<Id> = {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            let old = {
                let mut q = tx.prepare_cached("SELECT id FROM faces WHERE asset_id = ?1")?;
                q.query_map([asset], |r| r.get(0))?.collect::<rusqlite::Result<Vec<Id>>>()?
            };
            tx.execute("DELETE FROM faces WHERE asset_id = ?1", [asset])?;
            {
                let mut insert = tx.prepare_cached(
                    "INSERT INTO faces (asset_id, bbox, landmarks, det_score, quality, embedding)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for f in &found {
                    let bbox = format!("[{:.4},{:.4},{:.4},{:.4}]", f.bbox[0], f.bbox[1], f.bbox[2], f.bbox[3]);
                    let lm = f.landmarks.iter().map(|[x, y]| format!("[{x:.4},{y:.4}]")).collect::<Vec<_>>().join(",");
                    insert.execute(params![
                        asset,
                        bbox,
                        format!("[{lm}]"),
                        f.score,
                        f.quality,
                        faces::to_blob(&f.embedding)
                    ])?;
                    new_ids.push(Id(tx.last_insert_rowid()));
                }
            }
            tx.execute("UPDATE assets SET faces_version = ?2 WHERE id = ?1", params![asset, faces::PIPELINE_VERSION])?;
            tx.commit()?;
            old
        };

        let dir = self.faces_dir();
        for id in old_ids {
            let _ = std::fs::remove_file(faces::crop_path(&dir, id));
        }
        for (id, f) in new_ids.into_iter().zip(found) {
            write_crop(&faces::crop_path(&dir, id), &f.crop)?;
        }
        Ok(())
    }
}

/// Decodes a photo and applies its EXIF orientation.
fn decode_upright(
    path: &Path,
    data: Option<&[u8]>,
    min_short: u32,
    orientation: Option<u16>,
) -> Result<RgbImage, ThumbError> {
    let img = thumbs::decode_photo(path, data, min_short)?;
    Ok(thumbs::orient(img, orientation.unwrap_or(1)).into_rgb8())
}

fn write_crop(dest: &Path, crop: &RgbImage) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = webp::Encoder::from_rgb(crop.as_raw(), crop.width(), crop.height()).encode(FACE_CROP_QUALITY);
    std::fs::write(dest, &*encoded)
}

const CHUNK: usize = 64 * 1024;

/// XXH3 over size, the first and last 64 KB, and the capture time: identifies
/// a file after a move or rename without reading all of it.
fn fingerprint(path: &Path, size: i64, meta: &Meta) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = vec![0u8; CHUNK];
    let n = f.read(&mut buf)?;
    hasher.update(&buf[..n]);
    if size as usize > CHUNK * 2 {
        f.seek(SeekFrom::End(-(CHUNK as i64)))?;
        let n = f.read(&mut buf)?;
        hasher.update(&buf[..n]);
    }
    if let Some(t) = meta.taken_at {
        hasher.update(t.and_utc().timestamp().to_le_bytes().as_slice());
    }
    Ok(format!("{:016x}", hasher.digest()))
}

/// Same fingerprint as [`fingerprint`], from bytes already in memory.
fn fingerprint_bytes(data: &[u8], meta: &Meta) -> String {
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    hasher.update(&(data.len() as i64).to_le_bytes());
    hasher.update(&data[..data.len().min(CHUNK)]);
    if data.len() > CHUNK * 2 {
        hasher.update(&data[data.len() - CHUNK..]);
    }
    if let Some(t) = meta.taken_at {
        hasher.update(t.and_utc().timestamp().to_le_bytes().as_slice());
    }
    format!("{:016x}", hasher.digest())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn fingerprints_match_from_disk_and_memory() {
        let dir = tempfile::tempdir().unwrap();
        for len in [1_000usize, 200_000] {
            let p = dir.path().join(format!("f{len}"));
            std::fs::write(&p, (0..len).map(|i| (i % 251) as u8).collect::<Vec<_>>()).unwrap();
            let m = Meta::default();
            assert_eq!(fingerprint(&p, len as i64, &m).unwrap(), fingerprint_bytes(&std::fs::read(&p).unwrap(), &m));
        }
    }

    fn noisy(w: u32, h: u32) -> RgbImage {
        // Noise so the encoded files clear MIN_FILE_BYTES.
        RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 7 % 251) as u8, (y * 13 % 251) as u8, ((x ^ y) % 251) as u8]))
    }

    #[test]
    fn indexes_photos_and_records_failures() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        noisy(1600, 1200).save(photos.join("IMG-20210512-WA0003.jpg")).unwrap();
        noisy(400, 900).save(photos.join("portrait.png")).unwrap();
        std::fs::write(photos.join("broken.jpg"), vec![0u8; 20_000]).unwrap();

        let lib = Library::open(dir.path().join("lib")).unwrap();
        let src = lib.add_source(&photos).unwrap();
        lib.scan_source(&src, &AtomicBool::new(false)).unwrap();

        let calls = AtomicU32::new(0);
        let p = lib
            .run_index(2, &AtomicBool::new(false), &|_| {
                calls.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        assert_eq!((p.total, p.done, p.failed), (3, 2, 1));
        assert_eq!(calls.load(Ordering::Relaxed), 3);

        let conn = lib.read();
        let (taken, src_kind, w, h): (String, String, u32, u32) = conn
            .query_row(
                "SELECT taken_at, taken_at_source, width, height FROM assets WHERE rel_path LIKE 'IMG-%'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((taken.as_str(), src_kind.as_str(), w, h), ("2021-05-12T12:00:00", "filename", 1600, 1200));
        let thumbs_ok: u32 =
            conn.query_row("SELECT count(*) FROM assets WHERE thumb_state = 'ok'", [], |r| r.get(0)).unwrap();
        assert_eq!(thumbs_ok, 2);
        let retry: (String, i64) = conn
            .query_row("SELECT state, attempts FROM jobs WHERE stage = 'index'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(retry, ("queued".to_string(), 1));
        // Without models, decoded photos wait for face analysis.
        let waiting: u32 = conn.query_row("SELECT count(*) FROM jobs WHERE stage = 'faces'", [], |r| r.get(0)).unwrap();
        assert_eq!(waiting, 2);
    }

    #[test]
    fn analyses_faces_once_models_are_available() {
        let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        if faces::model_paths(&models).is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        noisy(1200, 900).save(photos.join("a.jpg")).unwrap();
        let lib = Library::open(dir.path().join("lib")).unwrap();
        let src = lib.add_source(&photos).unwrap();
        lib.scan_source(&src, &AtomicBool::new(false)).unwrap();
        lib.run_index(1, &AtomicBool::new(false), &|_| {}).unwrap();

        assert!(lib.use_models(&models));
        let p = lib.run_index(1, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!((p.total, p.done), (1, 1));
        let version: Option<i64> = lib.read().query_row("SELECT faces_version FROM assets", [], |r| r.get(0)).unwrap();
        assert_eq!(version, Some(faces::PIPELINE_VERSION));
    }
}
