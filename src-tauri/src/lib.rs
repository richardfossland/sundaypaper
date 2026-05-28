//! SundayPaper main library — Tauri runtime entry point.
//!
//! Phase 0 wires up only the bare bridge: tracing, the opener plugin, and the
//! `app_info` IPC command that the dashboard calls to prove Rust ↔ React works.
//!
//! Later phases add `AppState` (SQLite pool via sqlx, app-local data dir) in
//! the `setup` callback and register the real domain commands here. Command
//! implementations live in `commands::*` — this file only registers them.

pub mod commands;
pub mod error;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            tracing::info!("SundayPaper backend ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::app::app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
