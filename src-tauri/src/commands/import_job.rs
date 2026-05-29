//! Import-job IPC commands. Thin wrappers over `ImportJobRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::import_job::{ImportJob, ImportJobRepo};
use crate::AppState;

#[tauri::command]
pub async fn import_job_create(
    state: State<'_, AppState>,
    project_id: Option<String>,
    source_path: String,
    kind: String,
) -> AppResult<ImportJob> {
    ImportJobRepo::new(state.db.clone())
        .create(project_id.as_deref(), &source_path, &kind)
        .await
}

#[tauri::command]
pub async fn import_job_get(state: State<'_, AppState>, id: String) -> AppResult<ImportJob> {
    ImportJobRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn import_job_list(state: State<'_, AppState>) -> AppResult<Vec<ImportJob>> {
    ImportJobRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn import_job_update_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
    detail: Option<String>,
) -> AppResult<ImportJob> {
    ImportJobRepo::new(state.db.clone())
        .update_status(&id, &status, detail.as_deref())
        .await
}
