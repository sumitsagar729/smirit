//! Source folders: the user's photo directories that make up the library.

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Error, Id, Library, Result};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: Id,
    pub path: String,
    pub is_online: bool,
    pub added_at: String,
    pub last_scan_at: Option<String>,
    pub photos: u32,
    pub videos: u32,
}

impl Library {
    /// Adds a folder to the library. Rejects duplicates and folders nested
    /// inside (or containing) an existing source, so no file is indexed twice.
    pub fn add_source(&self, path: impl AsRef<Path>) -> Result<Source> {
        let path = normalize(path.as_ref())?;
        if path.starts_with(normalize(self.root()).unwrap_or_else(|_| self.root().to_path_buf())) {
            return Err(Error::SourceIsLibrary);
        }
        for existing in self.list_sources()? {
            let existing = PathBuf::from(existing.path);
            if existing == path {
                return Err(Error::SourceAlreadyAdded(path));
            }
            if path.starts_with(&existing) || existing.starts_with(&path) {
                return Err(Error::SourceOverlaps { new: path, existing });
            }
        }

        let id = {
            let conn = self.conn();
            conn.execute("INSERT INTO sources (path) VALUES (?1)", [path.to_string_lossy()])?;
            Id(conn.last_insert_rowid())
        };
        tracing::info!(%id, path = %path.display(), "source added");
        self.get_source(id)
    }

    pub fn remove_source(&self, id: Id) -> Result<()> {
        let n = self.conn().execute("DELETE FROM sources WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Error::NotFound(format!("source {id}")));
        }
        tracing::info!(%id, "source removed");
        Ok(())
    }

    pub fn list_sources(&self) -> Result<Vec<Source>> {
        let conn = self.read();
        let mut stmt = conn.prepare(&format!("{SELECT_SOURCE} GROUP BY s.id ORDER BY s.path"))?;
        let rows = stmt.query_map([], row_to_source)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_source(&self, id: Id) -> Result<Source> {
        let conn = self.conn();
        conn.query_row(&format!("{SELECT_SOURCE} WHERE s.id = ?1 GROUP BY s.id"), params![id], row_to_source)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("source {id}")))
    }
}

const SELECT_SOURCE: &str = "
    SELECT s.id, s.path, s.is_online, s.added_at, s.last_scan_at,
           count(CASE WHEN a.kind = 'photo' AND a.status = 'active' THEN 1 END),
           count(CASE WHEN a.kind = 'video' AND a.status = 'active' THEN 1 END)
    FROM sources s LEFT JOIN assets a ON a.source_id = s.id";

fn row_to_source(r: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: r.get(0)?,
        path: r.get(1)?,
        is_online: r.get(2)?,
        added_at: r.get(3)?,
        last_scan_at: r.get(4)?,
        photos: r.get(5)?,
        videos: r.get(6)?,
    })
}

/// Absolute, canonical path without Windows' `\\?\` prefix.
fn normalize(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        return Err(Error::FolderNotFound(path.to_path_buf()));
    }
    let canon = path.canonicalize()?;
    #[cfg(windows)]
    {
        let s = canon.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            if !stripped.starts_with("UNC\\") {
                return Ok(PathBuf::from(stripped));
            }
            return Ok(PathBuf::from(format!(r"\\{}", &stripped[4..])));
        }
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().join("lib")).unwrap();
        (dir, lib)
    }

    #[test]
    fn add_list_remove() {
        let (dir, lib) = setup();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();

        let s = lib.add_source(&photos).unwrap();
        assert_eq!((s.photos, s.videos), (0, 0));
        assert!(s.is_online);
        assert_eq!(lib.list_sources().unwrap().len(), 1);

        lib.remove_source(s.id).unwrap();
        assert!(lib.list_sources().unwrap().is_empty());
        assert!(matches!(lib.remove_source(s.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn rejects_duplicates_overlaps_and_missing() {
        let (dir, lib) = setup();
        let photos = dir.path().join("photos");
        let nested = photos.join("2021");
        std::fs::create_dir_all(&nested).unwrap();

        lib.add_source(&photos).unwrap();
        assert!(matches!(lib.add_source(&photos), Err(Error::SourceAlreadyAdded(_))));
        assert!(matches!(lib.add_source(&nested), Err(Error::SourceOverlaps { .. })));
        assert!(matches!(lib.add_source(dir.path().join("nope")), Err(Error::FolderNotFound(_))));
        assert!(matches!(lib.add_source(dir.path().join("lib")), Err(Error::SourceIsLibrary)));
        assert!(matches!(lib.add_source(dir.path()), Err(Error::SourceOverlaps { .. } | Error::SourceIsLibrary)));
    }
}
