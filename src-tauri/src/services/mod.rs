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
//!   - `bulletin` local ServicePlan mirror → program block specs
//!   - `bulletin_contract` published `sunday-contracts` ServicePlan → the local
//!     mirror → `bulletin` (the canonical Plan→Paper adapter; golden-fixture
//!     round-trip tested)
//!
//! Planned modules (added in their phases):
//!   - `ocr`    Tesseract pipeline for scanned songbooks (Phase 3.1)
//!
//! Phase 4.2:
//!   - `layout` Typst engine: block tree → PDF
//!
//! Phase 5.1:
//!   - `ai`     intent→layout compiler — free-text intent → BlockSpec tree via
//!     Claude tool-use, then into the existing layout pipeline. Pure
//!     request-builder/parser always on; the HTTP client behind the `ai`
//!     feature.

pub mod ai;
pub mod asset;
pub mod asset_lib;
pub mod block;
pub mod bulletin;
pub mod bulletin_contract;
pub mod db;
pub mod doc_template;
pub mod document;
pub mod export;
pub mod import_job;
pub mod layout;
pub mod pdf;
pub mod pdf_ops;
pub mod project;
pub mod sangbok;
pub mod setting;
pub mod song;
pub mod template;
