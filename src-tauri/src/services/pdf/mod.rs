//! PDF layer (Phase 1.2).
//!
//! `plan` (pure page-range / split / rotation logic) is always compiled and
//! unit-tested. The engine is behind the `pdf` feature: `edit` (lopdf —
//! split/merge/rotate/extract/info, round-trip tested) and `render` (pdfium —
//! rasterise + text extraction, compile-checked only).
//!
//! The free functions below are the single entry points the commands call.
//! When built with `--features pdf` they read the document, run the pure plan,
//! and drive the engine; otherwise they return `FeatureDisabled` so the
//! renderer can tell the user this build can't do PDF work — mirrors ADR-002.

pub mod plan;

#[cfg(feature = "pdf")]
mod edit;
#[cfg(feature = "pdf")]
mod render;

use std::path::Path;

use crate::error::AppResult;
pub use plan::{PdfInfo, PdfPageInfo};

#[cfg(feature = "pdf")]
mod engine {
    use super::*;
    use crate::services::pdf::plan::{normalize_rotation, parse_page_selection, plan_split_every};

    /// Page count + best-effort per-page sizes.
    pub fn info(path: &Path) -> AppResult<PdfInfo> {
        edit::info(path)
    }

    /// Full document text, pages joined by form-feed (`\f`).
    pub fn extract_text(path: &Path) -> AppResult<String> {
        render::extract_text(path)
    }

    /// Render a 0-based page to PNG bytes, scaled to `target_width` px.
    pub fn render_page_png(path: &Path, index: u16, target_width: u32) -> AppResult<Vec<u8>> {
        render::render_page_png(path, index, target_width)
    }

    /// Write a new PDF containing only the pages named by `spec`
    /// (e.g. `"1-3,5"`), in document order.
    pub fn extract_pages(path: &Path, spec: &str, out: &Path) -> AppResult<()> {
        let count = edit::info(path)?.page_count;
        let selection = parse_page_selection(spec, count)?;
        edit::extract(path, &selection, out)
    }

    /// Split into consecutive chunks of `chunk_size` pages, one file per chunk
    /// named `{stem}_NN.pdf` in `out_dir`. Returns the output paths.
    pub fn split_every(
        path: &Path,
        chunk_size: u32,
        out_dir: &Path,
        stem: &str,
    ) -> AppResult<Vec<String>> {
        let count = edit::info(path)?.page_count;
        let chunks = plan_split_every(count, chunk_size)?;
        edit::split(path, &chunks, out_dir, stem)
    }

    /// Merge `inputs` (in order) into one PDF at `out`.
    pub fn merge(inputs: &[String], out: &Path) -> AppResult<()> {
        edit::merge(inputs, out)
    }

    /// Rotate the pages named by `spec` by `degrees` (any multiple of 90).
    pub fn rotate(path: &Path, spec: &str, degrees: i64, out: &Path) -> AppResult<()> {
        let count = edit::info(path)?.page_count;
        let selection = parse_page_selection(spec, count)?;
        let degrees = normalize_rotation(degrees)?;
        edit::rotate(path, &selection, degrees, out)
    }
}

#[cfg(not(feature = "pdf"))]
mod engine {
    use super::*;
    use crate::error::AppError;

    fn disabled<T>() -> AppResult<T> {
        Err(AppError::FeatureDisabled { feature: "pdf" })
    }

    pub fn info(_path: &Path) -> AppResult<PdfInfo> {
        disabled()
    }
    pub fn extract_text(_path: &Path) -> AppResult<String> {
        disabled()
    }
    pub fn render_page_png(_path: &Path, _index: u16, _target_width: u32) -> AppResult<Vec<u8>> {
        disabled()
    }
    pub fn extract_pages(_path: &Path, _spec: &str, _out: &Path) -> AppResult<()> {
        disabled()
    }
    pub fn split_every(
        _path: &Path,
        _chunk_size: u32,
        _out_dir: &Path,
        _stem: &str,
    ) -> AppResult<Vec<String>> {
        disabled()
    }
    pub fn merge(_inputs: &[String], _out: &Path) -> AppResult<()> {
        disabled()
    }
    pub fn rotate(_path: &Path, _spec: &str, _degrees: i64, _out: &Path) -> AppResult<()> {
        disabled()
    }
}

pub use engine::{extract_pages, extract_text, info, merge, render_page_png, rotate, split_every};
