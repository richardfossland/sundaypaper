//! Tauri command handlers.
//!
//! Commands are the thin IPC layer the renderer calls via `invoke()`. They
//! delegate to `services::*` for real work and return `Result<T, AppError>`.
//! Naming convention: `entity_verb` (e.g. `app_info`, later `pdf_open`).

pub mod app;
