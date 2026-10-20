//! `smriti://` serves thumbnails and originals straight from disk, so the grid
//! never pushes image bytes through IPC.
//!
//!   smriti://localhost/thumb/<id>   WebP thumbnail
//!   smriti://localhost/media/<id>   original file, with HTTP Range for video seeking
//!   smriti://localhost/face/<id>    square face crop (WebP)
//!   smriti://localhost/geo/<name>   map outlines: `countries` or `states` (GeoJSON)
//!
//! (On Windows the webview addresses it as `http://smriti.localhost/...`.)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use smriti_core::{Id, faces, thumbs};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Manager, Runtime};

use crate::AppState;

const MAX_CHUNK: u64 = 4 * 1024 * 1024;

pub fn handle<R: Runtime>(app: &AppHandle<R>, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(state) = app.try_state::<AppState>() else { return status(StatusCode::SERVICE_UNAVAILABLE) };
    let path = req.uri().path().trim_start_matches('/');
    if let Some(name) = path.strip_prefix("geo/") {
        return geo(state.geo_dir.as_deref(), name);
    }
    let (kind, id) = match path.split_once('/') {
        Some((k, id)) => (k, id.parse::<i64>().ok().map(Id)),
        None => (path, None),
    };
    let Some(id) = id else { return status(StatusCode::BAD_REQUEST) };

    match kind {
        "thumb" | "face" => {
            let p = if kind == "thumb" {
                thumbs::thumb_path(&state.library.thumbs_dir(), id)
            } else {
                faces::crop_path(&state.library.faces_dir(), id)
            };
            match std::fs::read(&p) {
                Ok(bytes) => Response::builder()
                    .header(header::CONTENT_TYPE, "image/webp")
                    .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                    .body(bytes)
                    .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
                Err(_) => status(StatusCode::NOT_FOUND),
            }
        }
        "media" => match state.library.asset_file(id) {
            Ok((path, mime)) => {
                serve_file(&path, &mime, req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()))
                    .unwrap_or_else(|_| status(StatusCode::NOT_FOUND))
            }
            Err(_) => status(StatusCode::NOT_FOUND),
        },
        _ => status(StatusCode::NOT_FOUND),
    }
}

fn serve_file(path: &std::path::Path, mime: &str, range: Option<&str>) -> std::io::Result<Response<Vec<u8>>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let builder = Response::builder().header(header::CONTENT_TYPE, mime).header(header::ACCEPT_RANGES, "bytes");

    // `bytes=start-[end]`, clamped to MAX_CHUNK so long videos stream in pieces.
    let parsed = range.and_then(|r| r.strip_prefix("bytes=")).and_then(|r| {
        let (a, b) = r.split_once('-')?;
        let start: u64 = a.trim().parse().ok()?;
        let end: u64 = b.trim().parse().unwrap_or(len.saturating_sub(1));
        Some((start, end.min(len.saturating_sub(1))))
    });

    let response = match parsed {
        Some((start, end)) if start <= end && start < len => {
            let end = end.min(start + MAX_CHUNK - 1);
            let mut buf = vec![0u8; (end - start + 1) as usize];
            f.seek(SeekFrom::Start(start))?;
            f.read_exact(&mut buf)?;
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .header(header::CONTENT_LENGTH, buf.len())
                .body(buf)
        }
        Some(_) => {
            return Ok(Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                .body(Vec::new())
                .expect("valid response"));
        }
        None => {
            let mut buf = Vec::with_capacity(len as usize);
            f.read_to_end(&mut buf)?;
            builder.header(header::CONTENT_LENGTH, buf.len()).body(buf)
        }
    };
    Ok(response.expect("valid response"))
}

/// Only these two files, by name, so the path can't wander.
fn geo(dir: Option<&std::path::Path>, name: &str) -> Response<Vec<u8>> {
    let file = match name {
        "countries" => "ne_50m_admin_0_countries.geojson",
        "states" => "ne_50m_admin_1_states_provinces.geojson",
        _ => return status(StatusCode::NOT_FOUND),
    };
    match dir.map(|d| std::fs::read(d.join(file))) {
        Some(Ok(bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, "application/geo+json")
            .header(header::CACHE_CONTROL, "max-age=86400")
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        _ => status(StatusCode::NOT_FOUND),
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).expect("valid response")
}
