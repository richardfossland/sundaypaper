//! Business logic lives here, one module per concern. Commands stay thin and
//! delegate to these.
//!
//! Phase 1.1 modules:
//!   - `db`       SQLite connection pool + migrations + `now_ms`
//!   - `project`  reference repository (the data-layer pattern)
//!   - `document` child repository (project → documents)
//!
//! Planned modules (added in their phases):
//!   - `pdf`    render + text extraction via pdfium-render (Phase 1.2)
//!   - `ocr`    Tesseract pipeline for scanned songbooks (Phase 3.1)
//!   - `layout` Typst engine: block tree → PDF (Phase 4.2)
//!   - `ai`     hybrid local/Claude provider (Phase 5.1)

pub mod db;
pub mod document;
pub mod project;
