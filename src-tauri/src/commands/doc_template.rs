//! Document template IPC commands. Thin wrappers over `DocTemplateRepo`.

use std::collections::HashMap;

use tauri::State;

use crate::error::AppResult;
use crate::services::doc_template::{DocTemplate, DocTemplateRepo, TemplateVarInput};
use crate::AppState;

#[tauri::command]
pub async fn doc_template_create(
    state: State<'_, AppState>,
    name: String,
    kind: String,
    typst_source: Option<String>,
    variables: Option<Vec<TemplateVarInput>>,
) -> AppResult<DocTemplate> {
    DocTemplateRepo::new(state.db.clone())
        .create(
            &name,
            &kind,
            typst_source.as_deref().unwrap_or(""),
            variables.as_deref().unwrap_or(&[]),
        )
        .await
}

#[tauri::command]
pub async fn doc_template_get(state: State<'_, AppState>, id: String) -> AppResult<DocTemplate> {
    DocTemplateRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn doc_template_list(
    state: State<'_, AppState>,
    kind: Option<String>,
) -> AppResult<Vec<DocTemplate>> {
    DocTemplateRepo::new(state.db.clone())
        .list(kind.as_deref())
        .await
}

#[tauri::command]
pub async fn doc_template_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    kind: String,
    typst_source: String,
    variables: Option<Vec<TemplateVarInput>>,
) -> AppResult<DocTemplate> {
    DocTemplateRepo::new(state.db.clone())
        .update(
            &id,
            &name,
            &kind,
            &typst_source,
            variables.as_deref().unwrap_or(&[]),
        )
        .await
}

#[tauri::command]
pub async fn doc_template_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    DocTemplateRepo::new(state.db.clone()).delete(&id).await
}

/// Render a template by substituting `{{VAR}}` placeholders with `vars`.
/// Returns the Typst source string (not a compiled PDF).
#[tauri::command]
pub async fn doc_template_render(
    state: State<'_, AppState>,
    id: String,
    vars: HashMap<String, String>,
) -> AppResult<String> {
    DocTemplateRepo::new(state.db.clone())
        .render(&id, &vars)
        .await
}

/// Seed the three built-in templates if not already present.
#[tauri::command]
pub async fn doc_template_seed_builtins(state: State<'_, AppState>) -> AppResult<()> {
    DocTemplateRepo::new(state.db.clone()).seed_builtins().await
}
