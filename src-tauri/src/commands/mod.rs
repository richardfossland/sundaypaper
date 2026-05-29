//! Tauri command handlers.
//!
//! Commands are the thin IPC layer the renderer calls via `invoke()`. They
//! delegate to `services::*` for real work and return `Result<T, AppError>`.
//! Naming convention: `entity_verb` (e.g. `app_info`, later `pdf_open`).

pub mod app;
pub mod asset;
pub mod block;
pub mod document;
pub mod import_job;
pub mod pdf;
pub mod project;
pub mod setting;
pub mod song;
pub mod template;
