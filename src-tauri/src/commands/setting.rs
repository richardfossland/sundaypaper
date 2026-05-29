//! Setting IPC commands. Thin wrappers over `SettingRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::setting::{Setting, SettingRepo};
use crate::AppState;

#[tauri::command]
pub async fn setting_get(state: State<'_, AppState>, key: String) -> AppResult<Option<String>> {
    SettingRepo::new(state.db.clone()).get(&key).await
}

#[tauri::command]
pub async fn setting_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<Setting> {
    SettingRepo::new(state.db.clone()).set(&key, &value).await
}

#[tauri::command]
pub async fn setting_list(state: State<'_, AppState>) -> AppResult<Vec<Setting>> {
    SettingRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn setting_delete(state: State<'_, AppState>, key: String) -> AppResult<()> {
    SettingRepo::new(state.db.clone()).delete(&key).await
}
