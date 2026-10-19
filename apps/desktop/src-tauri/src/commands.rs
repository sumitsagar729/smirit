//! Commands the UI may call. Every input is validated in the core; errors
//! reach the UI as `{ code, message }`.

use serde::Serialize;
use smriti_core::timeline::{AssetDetail, Tile};
use smriti_core::{Id, LibraryStats, Source};
use tauri::State;

use crate::AppState;
use crate::indexing::IndexStatus;

#[derive(Debug, Serialize, specta::Type)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl From<smriti_core::Error> for ApiError {
    fn from(e: smriti_core::Error) -> Self {
        if matches!(e, smriti_core::Error::Db(_) | smriti_core::Error::Migration(_) | smriti_core::Error::Io(_)) {
            tracing::error!(error = %e, "command failed");
        }
        Self { code: e.code().to_string(), message: e.to_string() }
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn library_stats(state: State<'_, AppState>) -> ApiResult<LibraryStats> {
    Ok(state.library.stats()?)
}

#[tauri::command]
#[specta::specta]
pub fn list_sources(state: State<'_, AppState>) -> ApiResult<Vec<Source>> {
    Ok(state.library.list_sources()?)
}

#[tauri::command]
#[specta::specta]
pub fn add_source(state: State<'_, AppState>, path: String) -> ApiResult<Source> {
    let source = state.library.add_source(path)?;
    state.indexer.wake();
    Ok(source)
}

#[tauri::command]
#[specta::specta]
pub fn remove_source(state: State<'_, AppState>, id: Id) -> ApiResult<()> {
    Ok(state.library.remove_source(id)?)
}

/// Re-checks every folder for new, changed or removed files.
#[tauri::command]
#[specta::specta]
pub fn rescan(state: State<'_, AppState>) {
    state.indexer.wake();
}

#[tauri::command]
#[specta::specta]
pub fn index_status(state: State<'_, AppState>) -> IndexStatus {
    state.indexer.status()
}

/// Every visible photo and video, newest first. Async so the UI thread
/// never waits on the database.
#[tauri::command]
#[specta::specta]
pub async fn timeline(state: State<'_, AppState>) -> ApiResult<Vec<Tile>> {
    Ok(state.library.timeline()?)
}

#[tauri::command]
#[specta::specta]
pub async fn asset_detail(state: State<'_, AppState>, id: Id) -> ApiResult<AssetDetail> {
    Ok(state.library.asset_detail(id)?)
}
