//! Commands the UI may call. Every input is validated in the core; errors
//! reach the UI as `{ code, message }`.

use serde::Serialize;
use smriti_core::timeline::{AssetDetail, Tile};
use smriti_core::{
    AssetFace, FaceGroup, FaceRef, Id, LibraryStats, Person, PersonRef, PersonUpdate, Source, TimelineFilter,
};
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
pub async fn timeline(state: State<'_, AppState>, filter: Option<TimelineFilter>) -> ApiResult<Vec<Tile>> {
    Ok(state.library.timeline(filter.as_ref())?)
}

// ---- People ---------------------------------------------------------------
// Changes run recognition again in the background; the UI refetches when
// `IndexStatus.peopleGeneration` moves.

#[tauri::command]
#[specta::specta]
pub async fn people(state: State<'_, AppState>) -> ApiResult<Vec<Person>> {
    Ok(state.library.people()?)
}

#[tauri::command]
#[specta::specta]
pub async fn person(state: State<'_, AppState>, id: Id) -> ApiResult<Person> {
    Ok(state.library.person(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn face_groups(state: State<'_, AppState>, limit: u32) -> ApiResult<Vec<FaceGroup>> {
    Ok(state.library.face_groups(limit)?)
}

#[tauri::command]
#[specta::specta]
pub async fn group_faces(state: State<'_, AppState>, group: Id) -> ApiResult<Vec<FaceRef>> {
    Ok(state.library.group_faces(group)?)
}

#[tauri::command]
#[specta::specta]
pub async fn person_faces(state: State<'_, AppState>, person: Id, suggestions: bool) -> ApiResult<Vec<FaceRef>> {
    Ok(state.library.person_faces(person, suggestions)?)
}

#[tauri::command]
#[specta::specta]
pub async fn asset_faces(state: State<'_, AppState>, asset: Id) -> ApiResult<Vec<AssetFace>> {
    Ok(state.library.asset_faces(asset)?)
}

#[tauri::command]
#[specta::specta]
pub async fn name_group(state: State<'_, AppState>, group: Id, to: PersonRef) -> ApiResult<Id> {
    let id = state.library.name_group(group, &to)?;
    state.indexer.recognize();
    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn assign_faces(state: State<'_, AppState>, faces: Vec<Id>, to: PersonRef) -> ApiResult<Id> {
    let id = state.library.assign_faces(&faces, &to)?;
    state.indexer.recognize();
    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn reject_faces(state: State<'_, AppState>, faces: Vec<Id>) -> ApiResult<()> {
    state.library.reject_faces(&faces)?;
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn ignore_faces(state: State<'_, AppState>, faces: Vec<Id>) -> ApiResult<()> {
    state.library.ignore_faces(&faces)?;
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn ignore_group(state: State<'_, AppState>, group: Id) -> ApiResult<()> {
    state.library.ignore_group(group)?;
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_person(state: State<'_, AppState>, id: Id, update: PersonUpdate) -> ApiResult<()> {
    state.library.update_person(id, &update)?;
    // A birth date changes age-aware matching.
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn merge_people(state: State<'_, AppState>, from: Id, into: Id) -> ApiResult<()> {
    state.library.merge_people(from, into)?;
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_person(state: State<'_, AppState>, id: Id) -> ApiResult<()> {
    state.library.delete_person(id)?;
    state.indexer.recognize();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_cover_face(state: State<'_, AppState>, person: Id, face: Id) -> ApiResult<()> {
    Ok(state.library.set_cover_face(person, face)?)
}

#[tauri::command]
#[specta::specta]
pub async fn asset_detail(state: State<'_, AppState>, id: Id) -> ApiResult<AssetDetail> {
    Ok(state.library.asset_detail(id)?)
}
