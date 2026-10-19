//! Commands the UI may call. Every input is validated in the core; errors
//! reach the UI as `{ code, message }`.

use serde::Serialize;
use smriti_core::{Id, LibraryStats, Source};
use tauri::State;

use crate::AppState;

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
    Ok(state.library.add_source(path)?)
}

#[tauri::command]
#[specta::specta]
pub fn remove_source(state: State<'_, AppState>, id: Id) -> ApiResult<()> {
    Ok(state.library.remove_source(id)?)
}
