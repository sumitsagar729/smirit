//! Folder scanning: finds new, changed, missing and returning files and queues
//! them for indexing. Only directory entries are read here, never file contents.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use rusqlite::params;
use serde::Serialize;
use walkdir::WalkDir;

use crate::media::{self, Kind};
use crate::{Id, Library, Result, Source};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub files_seen: u32,
    pub added: u32,
    pub changed: u32,
    pub missing: u32,
    pub restored: u32,
    pub offline_sources: u32,
}

impl ScanReport {
    fn merge(&mut self, o: &ScanReport) {
        self.files_seen += o.files_seen;
        self.added += o.added;
        self.changed += o.changed;
        self.missing += o.missing;
        self.restored += o.restored;
        self.offline_sources += o.offline_sources;
    }
}

struct Found {
    rel: String,
    size: u64,
    mtime: i64,
    kind: Kind,
    mime: &'static str,
}

impl Library {
    pub fn scan_all(&self, cancel: &AtomicBool) -> Result<ScanReport> {
        let mut total = ScanReport::default();
        for source in self.list_sources()? {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            total.merge(&self.scan_source(&source, cancel)?);
        }
        Ok(total)
    }

    pub fn scan_source(&self, source: &Source, cancel: &AtomicBool) -> Result<ScanReport> {
        let root = PathBuf::from(&source.path);
        if !root.is_dir() {
            let conn = self.conn();
            conn.execute("UPDATE sources SET is_online = 0 WHERE id = ?1", [source.id])?;
            conn.execute(
                "UPDATE assets SET status = 'offline' WHERE source_id = ?1 AND status = 'active'",
                [source.id],
            )?;
            tracing::warn!(path = %root.display(), "source offline");
            return Ok(ScanReport { offline_sources: 1, ..Default::default() });
        }

        let started = std::time::Instant::now();
        let found = walk(&root, self.root(), cancel);
        if cancel.load(Ordering::Relaxed) {
            return Ok(ScanReport::default()); // partial walk: don't mark anything missing
        }
        let report = self.reconcile(source.id, &found)?;
        tracing::info!(path = %root.display(), ?report, ms = started.elapsed().as_millis() as u64, "source scanned");
        Ok(report)
    }

    fn reconcile(&self, source_id: Id, found: &[Found]) -> Result<ScanReport> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;

        let mut known: HashMap<String, (Id, i64, i64, String)> = HashMap::new();
        {
            let mut stmt = tx.prepare("SELECT id, rel_path, size, mtime, status FROM assets WHERE source_id = ?1")?;
            let rows =
                stmt.query_map([source_id], |r| Ok((r.get(1)?, (r.get(0)?, r.get(2)?, r.get(3)?, r.get(4)?))))?;
            for row in rows {
                let (rel, v) = row?;
                known.insert(rel, v);
            }
        }

        let mut report = ScanReport { files_seen: found.len() as u32, ..Default::default() };
        {
            let mut insert = tx.prepare(
                "INSERT INTO assets (source_id, rel_path, size, mtime, kind, mime) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut changed = tx.prepare(
                "UPDATE assets SET size = ?2, mtime = ?3, status = 'active', indexed_at = NULL, thumb_state = NULL
                 WHERE id = ?1",
            )?;
            let mut restore = tx.prepare("UPDATE assets SET status = 'active' WHERE id = ?1")?;
            // Newest files first: lower priority runs first.
            let mut queue = tx.prepare(
                "INSERT INTO jobs (asset_id, stage, priority) VALUES (?1, 'index', ?2)
                 ON CONFLICT (asset_id, stage) DO UPDATE SET state = 'queued', attempts = 0, last_error = NULL",
            )?;

            for f in found {
                match known.remove(&f.rel) {
                    None => {
                        insert.execute(params![source_id, f.rel, f.size as i64, f.mtime, f.kind.as_str(), f.mime])?;
                        queue.execute(params![tx.last_insert_rowid(), -f.mtime])?;
                        report.added += 1;
                    }
                    Some((id, size, mtime, _)) if size != f.size as i64 || mtime != f.mtime => {
                        changed.execute(params![id, f.size as i64, f.mtime])?;
                        queue.execute(params![id, -f.mtime])?;
                        report.changed += 1;
                    }
                    Some((id, _, _, status)) if status != "active" => {
                        restore.execute([id])?;
                        report.restored += 1;
                    }
                    Some(_) => {}
                }
            }
        }

        // Whatever is left was not on disk this time.
        for (id, _, _, status) in known.values() {
            if status == "active" {
                tx.execute("UPDATE assets SET status = 'missing' WHERE id = ?1", [id])?;
                tx.execute("DELETE FROM jobs WHERE asset_id = ?1", [id])?;
                report.missing += 1;
            }
        }
        tx.execute(
            "UPDATE sources SET is_online = 1, last_scan_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [source_id],
        )?;
        tx.commit()?;
        Ok(report)
    }
}

fn walk(root: &Path, library_root: &Path, cancel: &AtomicBool) -> Vec<Found> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root).follow_links(false).into_iter().filter_entry(|e| {
        if e.depth() == 0 || !e.file_type().is_dir() {
            return true;
        }
        let skip = e.file_name().to_str().is_some_and(media::ignored_dir) || e.path().starts_with(library_root);
        !skip
    });
    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(error = %e, "unreadable entry skipped");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Some((kind, mime)) = media::classify(entry.path()) else { continue };
        let Ok(md) = entry.metadata() else { continue };
        if md.len() < media::MIN_FILE_BYTES {
            continue;
        }
        let mtime =
            md.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
        let Ok(rel) = entry.path().strip_prefix(root) else { continue };
        out.push(Found { rel: rel.to_string_lossy().replace('\\', "/"), size: md.len(), mtime, kind, mime });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn detects_added_changed_missing_and_restored() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        file(&photos.join("2021/Diwali/IMG_1.jpg"), 20_000);
        file(&photos.join("2021/VID_1.mp4"), 30_000);
        file(&photos.join("2021/notes.txt"), 20_000); // not media
        file(&photos.join("tiny.jpg"), 100); // below MIN_FILE_BYTES
        file(&photos.join(".git/x.jpg"), 20_000); // ignored dir

        let lib = Library::open(dir.path().join("lib")).unwrap();
        let src = lib.add_source(&photos).unwrap();
        let none = AtomicBool::new(false);

        let r = lib.scan_source(&src, &none).unwrap();
        assert_eq!((r.files_seen, r.added), (2, 2));
        assert_eq!(lib.stats().unwrap().pending_jobs, 2);

        // unchanged rescan is a no-op
        assert_eq!(lib.scan_source(&src, &none).unwrap(), ScanReport { files_seen: 2, ..Default::default() });

        // change one, delete one
        file(&photos.join("2021/Diwali/IMG_1.jpg"), 25_000);
        let vid = photos.join("2021/VID_1.mp4");
        std::fs::rename(&vid, dir.path().join("away.mp4")).unwrap();
        let r = lib.scan_source(&src, &none).unwrap();
        assert_eq!((r.changed, r.missing), (1, 1));
        assert_eq!(lib.stats().unwrap().videos, 0);

        // it comes back
        std::fs::rename(dir.path().join("away.mp4"), &vid).unwrap();
        assert_eq!(lib.scan_source(&src, &none).unwrap().restored, 1);
    }

    #[test]
    fn unplugged_source_goes_offline() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("usb");
        file(&photos.join("a.jpg"), 20_000);
        let lib = Library::open(dir.path().join("lib")).unwrap();
        let src = lib.add_source(&photos).unwrap();
        lib.scan_source(&src, &AtomicBool::new(false)).unwrap();

        std::fs::remove_dir_all(&photos).unwrap();
        let r = lib.scan_source(&src, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.offline_sources, 1);
        assert!(!lib.get_source(src.id).unwrap().is_online);
    }
}
