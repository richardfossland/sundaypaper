//! Layout engine — block tree → Typst source → PDF (Phase 4.2).
//!
//! Two halves, same split as the `pdf` layer:
//!   - `markup` — pure block tree → Typst **source string**. Always compiled and
//!     exhaustively unit-tested; no compiler, no I/O.
//!   - `engine` — Typst **source string → PDF bytes**. The embedded compiler is
//!     heavy, so it sits behind the `typst` cargo feature; the default build
//!     compiles without it and `compile` returns a clear `feature_disabled`
//!     error. This is the final FORWARD-pipeline step:
//!
//! ```text
//! ServicePlan --build_bulletin--> BlockSpec[] --(persist)--> Block tree
//!   --build_typst_document--> Typst source --compile--> PDF bytes
//! ```

pub mod engine;
pub mod markup;
