//! PDF operations commands — backward-direction ingest helpers.
//!
//! These are thin wrappers over `services::pdf_ops` (which wraps the existing
//! `services::pdf` engine). The only command not already covered by
//! `commands::pdf` is `pdf_page_count` — a focused query used by the ingest
//! UI to display page counts before starting a split or extract operation.

use std::path::Path;

use tauri::State;

use crate::error::AppResult;
use crate::services::pdf_ops;
use crate::AppState;

/// Return the number of pages in the PDF at `path`.
///
/// When built without the `pdf` cargo feature this returns a
/// `feature_disabled` error the renderer can surface as "upgrade this build".
#[tauri::command]
pub async fn pdf_page_count(_state: State<'_, AppState>, path: String) -> AppResult<usize> {
    pdf_ops::pdf_page_count(Path::new(&path))
}
