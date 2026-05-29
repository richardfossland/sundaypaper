//! Project IPC commands. Thin wrappers over `ProjectRepo`; all the rules live
//! in the service. Command names follow `entity_verb`.

use tauri::State;

use crate::error::AppResult;
use crate::services::project::{Project, ProjectRepo};
use crate::AppState;

#[tauri::command]
pub async fn project_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> AppResult<Project> {
    ProjectRepo::new(state.db.clone())
        .create(&name, description.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn project_get(state: State<'_, AppState>, id: String) -> AppResult<Project> {
    ProjectRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn project_list(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    ProjectRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn project_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    description: Option<String>,
) -> AppResult<Project> {
    ProjectRepo::new(state.db.clone())
        .update(&id, &name, description.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn project_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    ProjectRepo::new(state.db.clone()).delete(&id).await
}
