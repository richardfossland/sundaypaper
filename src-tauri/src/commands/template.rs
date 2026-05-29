//! Template IPC commands. Thin wrappers over `TemplateRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::template::{Template, TemplateRepo};
use crate::AppState;

#[tauri::command]
pub async fn template_create(
    state: State<'_, AppState>,
    name: String,
    kind: String,
    source: Option<String>,
) -> AppResult<Template> {
    TemplateRepo::new(state.db.clone())
        .create(&name, &kind, source.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn template_get(state: State<'_, AppState>, id: String) -> AppResult<Template> {
    TemplateRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn template_list(state: State<'_, AppState>) -> AppResult<Vec<Template>> {
    TemplateRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn template_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    kind: String,
    source: String,
) -> AppResult<Template> {
    TemplateRepo::new(state.db.clone())
        .update(&id, &name, &kind, &source)
        .await
}

#[tauri::command]
pub async fn template_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    TemplateRepo::new(state.db.clone()).delete(&id).await
}
