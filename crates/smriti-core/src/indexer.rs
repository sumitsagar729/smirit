//! Background indexing: metadata, fingerprint and thumbnail for every queued
//! asset. Work survives restarts because the queue lives in the `jobs` table.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bytes::Bytes;
use crossbeam_channel::bounded;
use nom_exif::MediaParser;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::media::{self, Kind};
use crate::metadata::{self, Meta};
use crate::thumbs::{self, ThumbError};
use crate::{Id, Library, Result};

const BATCH: usize = 64;
/// Photos buffered ahead of the decoders (about 16 x 6 MB).
const READ_AHEAD: usize = 16;
const MAX_BUFFERED: i64 = 64 * 1024 * 1024;
const MAX_ATTEMPTS: i64 = 3;

#[derive(Debug, Clone, Copy, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    /// Items in this run (done + failed + still queued).
    pub total: u32,
    pub done: u32,
    pub failed: u32,
}

struct Job {
    id: i64,
    asset: Id,
    path: PathBuf,
    kind: Kind,
    mime: String,
    size: i64,
    mtime: i64,
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
        let (total, run_started): (u32, String) = self.conn().query_row(
            "SELECT count(*), strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM jobs WHERE state = 'queued'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if total == 0 {
            return Ok(IndexProgress::default());
        }
        let done = AtomicU32::new(0);
        let failed = AtomicU32::new(0);
        let report =
            || IndexProgress { total, done: done.load(Ordering::Relaxed), failed: failed.load(Ordering::Relaxed) };
        tracing::info!(total, workers, "indexing started");
        let started = std::time::Instant::now();

        // One thread reads files sequentially; workers decode from memory.
        // On spinning disks parallel reads thrash the head (measured: 7 readers
        // ran 3.5x slower than 1 reader feeding 7 decoders).
        let (tx, rx) = bounded::<(Job, Option<Bytes>)>(READ_AHEAD);
        std::thread::scope(|s| -> Result<()> {
            for _ in 0..workers.max(1) {
                let rx = rx.clone();
                s.spawn(|| {
                    let mut parser = MediaParser::new();
                    for (job, data) in rx {
                        let ok = self.process(&mut parser, &job, data);
                        (if ok { &done } else { &failed }).fetch_add(1, Ordering::Relaxed);
                        progress(report());
                    }
                });
            }
            drop(rx);
            // Feed workers until the queue is drained or cancelled.
            'feed: while !cancel.load(Ordering::Relaxed) {
                let batch = self.claim(BATCH, &run_started)?;
                if batch.is_empty() {
                    break;
                }
                for job in batch {
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
    fn claim(&self, n: usize, run_started: &str) -> Result<Vec<Job>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "UPDATE jobs SET state = 'running', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id IN (SELECT id FROM jobs WHERE state = 'queued' AND updated_at <= ?2
                          ORDER BY priority, id LIMIT ?1)
             RETURNING id, asset_id",
        )?;
        let claimed: Vec<(i64, Id)> = stmt
            .query_map(params![n as i64, run_started], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut q = conn.prepare(
            "SELECT s.path, a.rel_path, a.kind, a.mime, a.size, a.mtime
             FROM assets a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
        )?;
        let mut jobs = Vec::with_capacity(claimed.len());
        for (id, asset) in claimed {
            let row = q
                .query_row([asset], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                })
                .optional()?;
            match row {
                Some((root, rel, kind, mime, size, mtime)) => jobs.push(Job {
                    id,
                    asset,
                    path: Path::new(&root).join(rel),
                    kind: Kind::parse(&kind).unwrap_or(Kind::Photo),
                    mime: mime.unwrap_or_default(),
                    size,
                    mtime,
                }),
                None => {
                    conn.execute("DELETE FROM jobs WHERE id = ?1", [id])?;
                }
            }
        }
        Ok(jobs)
    }

    /// Returns false when the item failed (it is retried up to MAX_ATTEMPTS).
    fn process(&self, parser: &mut MediaParser, job: &Job, data: Option<Bytes>) -> bool {
        let meta = metadata::read(parser, &job.path, job.kind, job.mtime, data.as_ref());
        let fingerprint = match &data {
            Some(d) => Some(fingerprint_bytes(d, &meta)),
            None => fingerprint(&job.path, job.size, &meta).ok(),
        };

        let dest = thumbs::thumb_path(&self.thumbs_dir(), job.asset);
        let thumb = match job.kind {
            Kind::Photo if media::decodable_photo(&job.mime) => {
                thumbs::make_photo(&job.path, data.as_deref(), meta.orientation, &dest)
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

        let result = self.save(job, &meta, fingerprint, thumb_state, dims, error.as_deref());
        if let Err(e) = &result {
            tracing::warn!(path = %job.path.display(), error = %e, "could not save index result");
        }
        error.is_none() && result.is_ok()
    }

    fn save(
        &self,
        job: &Job,
        m: &Meta,
        fingerprint: Option<String>,
        thumb_state: &str,
        thumb_dims: Option<(u32, u32)>,
        error: Option<&str>,
    ) -> Result<()> {
        // Prefer header dimensions; fall back to the thumbnail's aspect ratio.
        let (w, h) = match (m.width, m.height, thumb_dims) {
            (Some(w), Some(h), _) => (Some(w), Some(h)),
            (_, _, Some((w, h))) => (Some(w), Some(h)),
            _ => (None, None),
        };
        let conn = self.conn();
        conn.execute(
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
        match error {
            None => conn.execute("DELETE FROM jobs WHERE id = ?1", [job.id])?,
            Some(err) => {
                tracing::debug!(path = %job.path.display(), error = err, "index item failed");
                conn.execute(
                    "UPDATE jobs SET attempts = attempts + 1, last_error = ?2,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         state = CASE WHEN attempts + 1 >= ?3 THEN 'failed' ELSE 'queued' END
                     WHERE id = ?1",
                    params![job.id, err, MAX_ATTEMPTS],
                )?
            }
        };
        Ok(())
    }
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

    #[test]
    fn indexes_photos_and_records_failures() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        // Noise so the encoded files clear MIN_FILE_BYTES.
        let noisy = |w, h| {
            RgbImage::from_fn(w, h, |x, y| {
                image::Rgb([(x * 7 % 251) as u8, (y * 13 % 251) as u8, ((x ^ y) % 251) as u8])
            })
        };
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
        let retry: (String, i64) =
            conn.query_row("SELECT state, attempts FROM jobs", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(retry, ("queued".to_string(), 1));
    }
}
