//! Layout engine — block tree → Typst source (Phase 4.2).
//!
//! Pure markup generation: a rendered block list + page metadata become a Typst
//! document string. No PDF is produced here (that's the `pdf` feature via
//! pdfium/Typst at the seam); this module is fully unit-testable string-building.

pub mod markup;
