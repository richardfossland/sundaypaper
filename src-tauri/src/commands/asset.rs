//! Asset IPC commands. Thin wrappers over `AssetRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::asset::{Asset, AssetInput, AssetRepo};
use crate::AppState;

#[tauri::command]
pub async fn asset_create(
    state: State<'_, AppState>,
    kind: String,
    name: String,
    path: String,
    mime: Option<String>,
    byte_size: Option<i64>,
    fingerprint: Option<String>,
) -> AppResult<Asset> {
    AssetRepo::new(state.db.clone())
        .create(AssetInput {
            kind: &kind,
            name: &name,
            path: &path,
            mime: mime.as_deref(),
            byte_size,
            fingerprint: fingerprint.as_deref(),
        })
        .await
}

#[tauri::command]
pub async fn asset_get(state: State<'_, AppState>, id: String) -> AppResult<Asset> {
    AssetRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn asset_list(state: State<'_, AppState>) -> AppResult<Vec<Asset>> {
    AssetRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn asset_find_by_fingerprint(
    state: State<'_, AppState>,
    fingerprint: String,
) -> AppResult<Option<Asset>> {
    AssetRepo::new(state.db.clone())
        .find_by_fingerprint(&fingerprint)
        .await
}

#[tauri::command]
pub async fn asset_relink(
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> AppResult<Asset> {
    AssetRepo::new(state.db.clone()).relink(&id, &path).await
}

#[tauri::command]
pub async fn asset_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    AssetRepo::new(state.db.clone()).delete(&id).await
}
