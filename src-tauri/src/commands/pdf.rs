//! PDF IPC commands (Phase 1.2). Thin wrappers over `services::pdf`. When the
//! build lacks the `pdf` feature these return a `feature_disabled` error the
//! renderer can surface cleanly.
//!
//! Paths cross the boundary as strings; rendered pages cross as a base64 PNG so
//! the renderer can drop them straight into an `<img src="data:image/png...">`.

use std::path::Path;

use tauri::State;

use crate::error::AppResult;
use crate::services::pdf::{self, PdfInfo};
use crate::AppState;

#[tauri::command]
pub async fn pdf_info(_state: State<'_, AppState>, path: String) -> AppResult<PdfInfo> {
    pdf::info(Path::new(&path))
}

#[tauri::command]
pub async fn pdf_extract_text(_state: State<'_, AppState>, path: String) -> AppResult<String> {
    pdf::extract_text(Path::new(&path))
}

/// Render a 0-based page to a base64-encoded PNG (no data-URL prefix).
#[tauri::command]
pub async fn pdf_render_page(
    _state: State<'_, AppState>,
    path: String,
    page_index: u16,
    target_width: u32,
) -> AppResult<String> {
    let bytes = pdf::render_page_png(Path::new(&path), page_index, target_width)?;
    Ok(base64_encode(&bytes))
}

#[tauri::command]
pub async fn pdf_extract_pages(
    _state: State<'_, AppState>,
    path: String,
    pages: String,
    out_path: String,
) -> AppResult<()> {
    pdf::extract_pages(Path::new(&path), &pages, Path::new(&out_path))
}

#[tauri::command]
pub async fn pdf_split(
    _state: State<'_, AppState>,
    path: String,
    chunk_size: u32,
    out_dir: String,
    stem: String,
) -> AppResult<Vec<String>> {
    pdf::split_every(Path::new(&path), chunk_size, Path::new(&out_dir), &stem)
}

#[tauri::command]
pub async fn pdf_merge(
    _state: State<'_, AppState>,
    inputs: Vec<String>,
    out_path: String,
) -> AppResult<()> {
    pdf::merge(&inputs, Path::new(&out_path))
}

#[tauri::command]
pub async fn pdf_rotate(
    _state: State<'_, AppState>,
    path: String,
    pages: String,
    degrees: i64,
    out_path: String,
) -> AppResult<()> {
    pdf::rotate(Path::new(&path), &pages, degrees, Path::new(&out_path))
}

/// Minimal standard-base64 encoder (no padding-free, no deps) for PNG bytes.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 0x3F) as usize] as char);
        out.push(TABLE[(n >> 12 & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
