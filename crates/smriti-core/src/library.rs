use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, RwLock};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::migrations::{MIGRATIONS, latest_version};
use crate::{Result, faces, thumbs};

const DB_FILE: &str = "library.db";
const SUBDIRS: [&str; 4] = ["thumbs", "faces", "models", "backups"];
const KEEP_BACKUPS: usize = 7;

/// An open Smriti library: one SQLite database plus caches in `root`.
///
/// Two connections: `writer` for the indexer and edits, `reader` for the UI,
/// so browsing never waits behind indexing (WAL allows both at once).
pub struct Library {
    root: PathBuf,
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
    ffmpeg: Option<PathBuf>,
    models: RwLock<Option<PathBuf>>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub root: String,
    pub schema_version: u32,
    pub sources: u32,
    pub photos: u32,
    pub videos: u32,
    pub people: u32,
    pub pending_jobs: u32,
}

impl Library {
    /// Opens (or creates) the library at `root`, backing up and migrating the
    /// database when its schema is older than this build.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        for dir in SUBDIRS {
            fs::create_dir_all(root.join(dir))?;
        }

        let db_path = root.join(DB_FILE);
        let existed = db_path.exists();
        let mut conn = Connection::open(&db_path)?;
        configure(&conn)?;

        let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if existed && current < latest_version() {
            backup(&conn, &root, &format!("pre-v{}", latest_version()))?;
        }
        MIGRATIONS.to_latest(&mut conn)?;
        // Jobs left "running" by a crash or forced quit go back in the queue.
        conn.execute("UPDATE jobs SET state = 'queued' WHERE state = 'running'", [])?;

        let reader = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI,
        )?;
        reader.busy_timeout(std::time::Duration::from_secs(5))?;

        let ffmpeg = thumbs::find_ffmpeg();
        let models = std::env::var_os("SMRITI_MODELS")
            .map(PathBuf::from)
            .into_iter()
            .chain([root.join("models")])
            .find(|d| faces::model_paths(d).is_some());
        tracing::info!(root = %root.display(), from = current, to = latest_version(), ffmpeg = ?ffmpeg, models = ?models, "library opened");
        Ok(Self { root, writer: Mutex::new(conn), reader: Mutex::new(reader), ffmpeg, models: RwLock::new(models) })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.root.join("thumbs")
    }

    pub fn faces_dir(&self) -> PathBuf {
        self.root.join("faces")
    }

    /// Folder holding the face models, once they are available.
    pub fn models_dir(&self) -> Option<PathBuf> {
        self.models.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Uses the face models in `dir` (for example the app's bundled copy).
    /// Returns false when they are missing there.
    pub fn use_models(&self, dir: &Path) -> bool {
        let ok = faces::model_paths(dir).is_some();
        if ok {
            *self.models.write().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_path_buf());
        }
        ok
    }

    pub fn ffmpeg(&self) -> Option<&Path> {
        self.ffmpeg.as_deref()
    }

    // A poisoned lock only means another thread panicked mid-query;
    // SQLite itself stays consistent, so keep going.
    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        self.writer.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn read(&self) -> MutexGuard<'_, Connection> {
        self.reader.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn stats(&self) -> Result<LibraryStats> {
        let conn = self.read();
        let count = |sql: &str| -> Result<u32> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
        Ok(LibraryStats {
            root: self.root.display().to_string(),
            schema_version: conn.pragma_query_value(None, "user_version", |r| r.get(0))?,
            sources: count("SELECT count(*) FROM sources")?,
            photos: count("SELECT count(*) FROM assets WHERE kind = 'photo' AND status = 'active'")?,
            videos: count("SELECT count(*) FROM assets WHERE kind = 'video' AND status = 'active'")?,
            people: count("SELECT count(*) FROM persons WHERE name IS NOT NULL AND hidden = 0")?,
            pending_jobs: count("SELECT count(*) FROM jobs WHERE state != 'failed'")?,
        })
    }

    /// Writes a consistent snapshot to `backups/` and prunes old ones.
    pub fn backup(&self, label: &str) -> Result<PathBuf> {
        backup(&self.conn(), &self.root, label)
    }
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn backup(conn: &Connection, root: &Path, label: &str) -> Result<PathBuf> {
    let dir = root.join("backups");
    fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("library-{stamp}-{label}.db"));
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
    prune_backups(&dir)?;
    Ok(path)
}

fn prune_backups(dir: &Path) -> Result<()> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "db"))
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(KEEP_BACKUPS);
    for old in &files[..excess] {
        fs::remove_file(old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_layout_and_reports_empty_stats() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path()).unwrap();
        for sub in SUBDIRS {
            assert!(dir.path().join(sub).is_dir());
        }
        let s = lib.stats().unwrap();
        assert_eq!((s.sources, s.photos, s.videos, s.people), (0, 0, 0, 0));
        assert_eq!(s.schema_version, latest_version());
    }

    #[test]
    fn reopening_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        drop(Library::open(dir.path()).unwrap());
        let lib = Library::open(dir.path()).unwrap();
        assert_eq!(lib.stats().unwrap().schema_version, latest_version());
    }

    #[test]
    fn backups_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path()).unwrap();
        for i in 0..(KEEP_BACKUPS + 3) {
            lib.backup(&format!("t{i:02}")).unwrap();
        }
        let n = fs::read_dir(dir.path().join("backups")).unwrap().count();
        assert_eq!(n, KEEP_BACKUPS);
    }
}
