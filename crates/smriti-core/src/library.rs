use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use serde::Serialize;

use crate::Result;
use crate::migrations::{MIGRATIONS, latest_version};

const DB_FILE: &str = "library.db";
const SUBDIRS: [&str; 4] = ["thumbs", "faces", "models", "backups"];
const KEEP_BACKUPS: usize = 7;

/// An open Smriti library: one SQLite database plus caches in `root`.
pub struct Library {
    root: PathBuf,
    conn: Mutex<Connection>,
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
        tracing::info!(root = %root.display(), from = current, to = latest_version(), "library opened");

        Ok(Self { root, conn: Mutex::new(conn) })
    }

    /// Opens a throwaway in-memory library (tests).
    pub fn open_in_memory(root: impl AsRef<Path>) -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        configure(&conn)?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self { root: root.as_ref().to_path_buf(), conn: Mutex::new(conn) })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        // A poisoned lock only means another thread panicked mid-query;
        // SQLite itself stays consistent, so keep going.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn stats(&self) -> Result<LibraryStats> {
        let conn = self.conn();
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
