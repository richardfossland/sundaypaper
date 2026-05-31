//! SundayPaper main library — Tauri runtime entry point.
//!
//! Phase 1 wires the data layer: the `setup` callback opens the SQLite store in
//! the app-local data dir (running migrations from `sql/`) and stores it in
//! `AppState` so commands can reach it via `tauri::State`. Command
//! implementations live in `commands::*` — this file only registers them.
//!
//! Later phases add domain services (pdf, ocr, layout, ai) and their commands.

pub mod commands;
pub mod error;
pub mod services;

use tauri::Manager;

use services::db::Db;

/// Shared application state, managed by Tauri and injected into commands.
pub struct AppState {
    pub db: Db,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    // Auto-update + relaunch are desktop-only.
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .setup(|app| {
            // Open (and migrate) the local store before the UI can issue IPC.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("sundaypaper.db");
            let db = tauri::async_runtime::block_on(Db::connect_file(&db_path))?;
            app.manage(AppState { db });
            tracing::info!(path = %db_path.display(), "SundayPaper backend ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::project::project_create,
            commands::project::project_get,
            commands::project::project_list,
            commands::project::project_update,
            commands::project::project_delete,
            commands::document::document_create,
            commands::document::document_get,
            commands::document::document_list,
            commands::document::document_update,
            commands::document::document_delete,
            commands::block::block_create,
            commands::block::block_get,
            commands::block::block_list,
            commands::block::block_update,
            commands::block::block_delete,
            commands::bulletin::bulletin_generate,
            commands::asset::asset_create,
            commands::asset::asset_get,
            commands::asset::asset_list,
            commands::asset::asset_find_by_fingerprint,
            commands::asset::asset_relink,
            commands::asset::asset_delete,
            commands::song::song_create,
            commands::song::song_get,
            commands::song::song_list,
            commands::song::song_update,
            commands::song::song_delete,
            commands::template::template_create,
            commands::template::template_get,
            commands::template::template_list,
            commands::template::template_update,
            commands::template::template_delete,
            commands::import_job::import_job_create,
            commands::import_job::import_job_get,
            commands::import_job::import_job_list,
            commands::import_job::import_job_update_status,
            commands::setting::setting_get,
            commands::setting::setting_set,
            commands::setting::setting_list,
            commands::setting::setting_delete,
            commands::pdf::pdf_info,
            commands::pdf::pdf_extract_text,
            commands::pdf::pdf_render_page,
            commands::pdf::pdf_extract_pages,
            commands::pdf::pdf_split,
            commands::pdf::pdf_merge,
            commands::pdf::pdf_rotate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
