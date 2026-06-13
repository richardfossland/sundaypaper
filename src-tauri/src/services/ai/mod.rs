//! AI intent→layout compiler (Phase 5.1) — the unbuilt headline made real.
//!
//! Turns a free-text intent ("lag søndagens program for 1. søndag i advent med
//! to salmer og dåp") into a **populated block tree** — not decorated text —
//! that flows straight into the proven FORWARD pipeline:
//!
//! ```text
//! intent --ai::prompt::build_request--> Anthropic tool-use request
//!        --ai::client::compile_intent--> Claude (emit_block_tree tool)
//!        --ai::parse::parse_block_tree--> BlockSpec[]  (validated, in-catalogue)
//!        --(persist)--> Block tree --build_typst_document--> Typst --> PDF
//! ```
//!
//! The AI ONLY emits the tree; everything downstream is the existing, tested
//! `bulletin` → `layout::markup` → Typst code, reused entirely. The split mirrors
//! the rest of the crate (`pdf`, `layout`):
//!   - `prompt` — PURE request builder (system prompt + tool schema + intent).
//!     Always compiled, exhaustively unit-tested, no key/network.
//!   - `parse`  — PURE response validator (tool-use JSON → `BlockSpec[]`,
//!     constrained to the block catalogue). Always compiled, unit-tested with
//!     canned fixtures.
//!   - `client` — the impure HTTP transport behind the `ai` cargo feature. The
//!     default build compiles without `reqwest` and the stub returns a clear
//!     `feature_disabled` ("AI ikke aktivert") error, so the manual builder is
//!     unaffected and the gate is green with no key.
//!
//! Privacy (CLAUDE.md, promise #4): cloud AI is opt-in (consent-gated at the
//! command layer) and the payload carries only the intent plus the church/date
//! the operator chose to share — form and member content is excluded by
//! construction.

pub mod client;
pub mod parse;
pub mod prompt;
