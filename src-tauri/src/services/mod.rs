//! Business logic lives here, one module per concern. Commands stay thin and
//! delegate to these.
//!
//! Phase 1.1 modules — the data layer (`db` + one repository per entity):
//!   - `db`          SQLite connection pool + migrations + `now_ms`
//!   - `project`     reference repository (the data-layer pattern)
//!   - `document`    project → documents
//!   - `block`       document → block tree (self-referential)
//!   - `asset`       library files + fingerprint relink
//!   - `song`        catalog (carries `tono_work_id`)
//!   - `template`    reusable layout templates
//!   - `import_job`  backward-direction ingest job log
//!   - `setting`     local key/value store
//!
//! Phase 1.2:
//!   - `pdf`  read/render/manipulate PDFs; pure planning always on, the lopdf +
//!     pdfium engine behind the `pdf` feature
//!
//! Bridges (pure, always-on):
//!   - `bulletin`  ServicePlan (from SundayPlan) → program block specs
//!
//! Planned modules (added in their phases):
//!   - `ocr`    Tesseract pipeline for scanned songbooks (Phase 3.1)
//!   - `layout` Typst engine: block tree → PDF (Phase 4.2)
//!   - `ai`     hybrid local/Claude provider (Phase 5.1)

pub mod asset;
pub mod asset_lib;
pub mod block;
pub mod bulletin;
pub mod db;
pub mod doc_template;
pub mod document;
pub mod import_job;
pub mod layout;
pub mod pdf;
pub mod pdf_ops;
pub mod project;
pub mod sangbok;
pub mod setting;
pub mod song;
pub mod template;
