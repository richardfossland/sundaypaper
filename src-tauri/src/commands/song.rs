//! Song IPC commands. Thin wrappers over `SongRepo`.

use tauri::State;

use crate::error::AppResult;
use crate::services::song::{Song, SongInput, SongRepo};
use crate::AppState;

#[tauri::command]
pub async fn song_create(
    state: State<'_, AppState>,
    title: String,
    author: Option<String>,
    body: Option<String>,
    language: Option<String>,
    tono_work_id: Option<String>,
) -> AppResult<Song> {
    SongRepo::new(state.db.clone())
        .create(SongInput {
            title: &title,
            author: author.as_deref(),
            body: body.as_deref().unwrap_or(""),
            language: language.as_deref(),
            tono_work_id: tono_work_id.as_deref(),
        })
        .await
}

#[tauri::command]
pub async fn song_get(state: State<'_, AppState>, id: String) -> AppResult<Song> {
    SongRepo::new(state.db.clone()).get(&id).await
}

#[tauri::command]
pub async fn song_list(state: State<'_, AppState>) -> AppResult<Vec<Song>> {
    SongRepo::new(state.db.clone()).list().await
}

#[tauri::command]
pub async fn song_update(
    state: State<'_, AppState>,
    id: String,
    title: String,
    author: Option<String>,
    body: Option<String>,
    language: Option<String>,
    tono_work_id: Option<String>,
) -> AppResult<Song> {
    SongRepo::new(state.db.clone())
        .update(
            &id,
            SongInput {
                title: &title,
                author: author.as_deref(),
                body: body.as_deref().unwrap_or(""),
                language: language.as_deref(),
                tono_work_id: tono_work_id.as_deref(),
            },
        )
        .await
}

#[tauri::command]
pub async fn song_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    SongRepo::new(state.db.clone()).delete(&id).await
}
