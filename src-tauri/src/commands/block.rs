//! Block IPC commands. Thin wrappers over `BlockRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::block::{Block, BlockRepo};
use crate::AppState;

#[tauri::command]
pub async fn block_create(
    state: State<'_, AppState>,
    document_id: String,
    parent_id: Option<String>,
    kind: String,
    data: Option<String>,
) -> AppResult<Block> {
    BlockRepo::new(state.db.clone())
        .create(
            &document_id,
            parent_id.as_deref(),
            &kind,
            data.as_deref().unwrap_or(""),
        )
        .await
}

#[tauri::command]
pub async fn block_get(state: State<'_, AppState>, id: String) -> AppResult<Block> {
    BlockRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn block_list(state: State<'_, AppState>, document_id: String) -> AppResult<Vec<Block>> {
    BlockRepo::new(state.db.clone())
        .list_by_document(&document_id)
        .await
}

#[tauri::command]
pub async fn block_update(
    state: State<'_, AppState>,
    id: String,
    kind: String,
    data: String,
) -> AppResult<Block> {
    BlockRepo::new(state.db.clone())
        .update(&id, &kind, &data)
        .await
}

#[tauri::command]
pub async fn block_reorder(
    state: State<'_, AppState>,
    id: String,
    new_position: i64,
) -> AppResult<Block> {
    BlockRepo::new(state.db.clone())
        .reorder(&id, new_position)
        .await
}

#[tauri::command]
pub async fn block_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    BlockRepo::new(state.db.clone()).delete(&id).await
}
