//! Read models for browsing: the timeline grid and the viewer's info panel.

use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::search::Query;
use crate::{Error, Id, Kind, Library, Result};

/// One grid cell. Kept tiny: the whole library is sent to the UI at once
/// (36k tiles, about 2 MB) so scrolling and the year scrubber never wait.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub id: Id,
    /// Display aspect: width / height (1.0 until metadata is read). Never NaN.
    pub aspect: f32,
    /// Local capture time `YYYY-MM-DDTHH:MM:SS`, or null until indexed.
    pub taken_at: Option<String>,
    pub kind: Kind,
    /// Video length in seconds.
    pub duration: Option<f32>,
    pub has_thumb: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetail {
    pub id: Id,
    pub kind: Kind,
    pub path: String,
    pub file_name: String,
    pub folder: String,
    pub mime: Option<String>,
    pub size_bytes: f64,
    pub taken_at: Option<String>,
    pub taken_at_source: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub camera: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub duration: Option<f32>,
}

impl Library {
    /// Visible photos and videos matching `query` (everything when `None`), newest first.
    pub fn timeline(&self, query: Option<&Query>) -> Result<Vec<Tile>> {
        let (filter, args) = query.map(Query::to_sql).unwrap_or_default();
        let sql = format!(
            "SELECT id, width, height, taken_at, kind, duration_ms, thumb_state = 'ok'
             FROM assets
             WHERE status = 'active' AND is_hidden = 0{filter}
             ORDER BY taken_at IS NULL, taken_at DESC, id DESC"
        );
        let conn = self.read();
        let mut stmt = conn.prepare_cached(&sql)?;
        let tiles = stmt
            .query_map(rusqlite::params_from_iter(args), |r| {
                let w: Option<u32> = r.get(1)?;
                let h: Option<u32> = r.get(2)?;
                let aspect = match (w, h) {
                    (Some(w), Some(h)) if w > 0 && h > 0 => (w as f32 / h as f32).clamp(0.25, 4.0),
                    _ => 1.0,
                };
                let kind: String = r.get(4)?;
                Ok(Tile {
                    id: r.get(0)?,
                    aspect,
                    taken_at: r.get(3)?,
                    kind: Kind::parse(&kind).unwrap_or(Kind::Photo),
                    duration: r.get::<_, Option<i64>>(5)?.map(|ms| ms as f32 / 1000.0),
                    has_thumb: r.get::<_, Option<bool>>(6)?.unwrap_or(false),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tiles)
    }

    pub fn asset_detail(&self, id: Id) -> Result<AssetDetail> {
        let conn = self.read();
        conn.query_row(
            "SELECT a.id, a.kind, s.path, a.rel_path, a.mime, a.size, a.taken_at, a.taken_at_source,
                    a.width, a.height, a.camera, a.lat, a.lon, a.duration_ms
             FROM assets a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
            [id],
            |r| {
                let root: String = r.get(2)?;
                let rel: String = r.get(3)?;
                let full = Path::new(&root).join(&rel);
                let kind: String = r.get(1)?;
                Ok(AssetDetail {
                    id: r.get(0)?,
                    kind: Kind::parse(&kind).unwrap_or(Kind::Photo),
                    file_name: full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    folder: full.parent().map(|p| p.display().to_string()).unwrap_or_default(),
                    path: full.display().to_string(),
                    mime: r.get(4)?,
                    size_bytes: r.get::<_, i64>(5)? as f64,
                    taken_at: r.get(6)?,
                    taken_at_source: r.get(7)?,
                    width: r.get(8)?,
                    height: r.get(9)?,
                    camera: r.get(10)?,
                    lat: r.get(11)?,
                    lon: r.get(12)?,
                    duration: r.get::<_, Option<i64>>(13)?.map(|ms| ms as f32 / 1000.0),
                })
            },
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("asset {id}")))
    }

    /// Absolute path and MIME type of an original, for the media protocol.
    pub fn asset_file(&self, id: Id) -> Result<(PathBuf, String)> {
        let conn = self.read();
        conn.query_row(
            "SELECT s.path, a.rel_path, coalesce(a.mime, 'application/octet-stream')
             FROM assets a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
            [id],
            |r| Ok((Path::new(&r.get::<_, String>(0)?).join(r.get::<_, String>(1)?), r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("asset {id}")))
    }
}
