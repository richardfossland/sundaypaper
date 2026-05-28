//! Business logic lives here, one module per concern. Commands stay thin and
//! delegate to these.
//!
//! Planned modules (added in their phases):
//!   - `db`     SQLite connection + migrations (Phase 1.1)
//!   - `pdf`    render + text extraction via pdfium-render (Phase 1.2)
//!   - `ocr`    Tesseract pipeline for scanned songbooks (Phase 3.1)
//!   - `layout` Typst engine: block tree → PDF (Phase 4.2)
//!   - `ai`     hybrid local/Claude provider (Phase 5.1)
//!
//! Phase 0 has no services yet — the only command (`app_info`) is self-contained.
