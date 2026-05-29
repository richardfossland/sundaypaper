//! Document IPC commands. Thin wrappers over `DocumentRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::document::{Document, DocumentRepo};
use crate::AppState;

#[tauri::command]
pub async fn document_create(
    state: State<'_, AppState>,
    project_id: String,
    title: String,
    kind: String,
    page_size: Option<String>,
) -> AppResult<Document> {
    DocumentRepo::new(state.db.clone())
        .create(
            &project_id,
            &title,
            &kind,
            page_size.as_deref().unwrap_or("A4"),
        )
        .await
}

#[tauri::command]
pub async fn document_get(state: State<'_, AppState>, id: String) -> AppResult<Document> {
    DocumentRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn document_list(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<Document>> {
    DocumentRepo::new(state.db.clone())
        .list_by_project(&project_id)
        .await
}

#[tauri::command]
pub async fn document_update(
    state: State<'_, AppState>,
    id: String,
    title: String,
    kind: String,
    page_size: String,
) -> AppResult<Document> {
    DocumentRepo::new(state.db.clone())
        .update(&id, &title, &kind, &page_size)
        .await
}

#[tauri::command]
pub async fn document_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    DocumentRepo::new(state.db.clone()).delete(&id).await
}
