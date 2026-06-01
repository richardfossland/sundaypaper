//! Sangbok-klipper IPC commands. Thin wrappers over `SangbokRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::sangbok::{SangbokJob, SangbokRepo};
use crate::AppState;

/// Queue a new sangbok import job for `pdf_path`.
#[tauri::command]
pub async fn sangbok_import(
    state: State<'_, AppState>,
    pdf_path: String,
) -> AppResult<SangbokJob> {
    let repo = SangbokRepo::new(state.db.clone());
    let job = repo.import(&pdf_path).await?;
    // Immediately kick off the stub processor (fire-and-forget equivalent —
    // in the real implementation this would be an async task; here the stub
    // completes synchronously so we return the final state).
    repo.process(&job.id).await
}

/// List all sangbok jobs, newest first.
#[tauri::command]
pub async fn sangbok_list_jobs(state: State<'_, AppState>) -> AppResult<Vec<SangbokJob>> {
    SangbokRepo::new(state.db.clone()).list().await
}

/// Fetch a single job by id.
#[tauri::command]
pub async fn sangbok_get_job(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<SangbokJob> {
    SangbokRepo::new(state.db.clone()).get(&id).await
}

/// Cancel a job that is still running (Queued or Processing).
#[tauri::command]
pub async fn sangbok_cancel(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<SangbokJob> {
    SangbokRepo::new(state.db.clone()).cancel(&id).await
}
