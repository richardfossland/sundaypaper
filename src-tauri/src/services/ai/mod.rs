//! Intent→Layout AI compiler (Phase 5) — free-text intent → a populated block
//! tree, via the Anthropic Messages API.
//!
//! The product promise (CLAUDE.md #2) is that "AI compiles intent into a
//! document — it does not just decorate". A volunteer types something like
//! *"lag søndagens program for 1. søndag i advent med to salmer og dåp"* and
//! gets back an ordered list of [`BlockSpec`](crate::services::bulletin::BlockSpec)s
//! — exactly the shape `bulletin_generate` already persists and renders. The AI
//! ONLY emits the tree; it never touches the database, never persists, never
//! bypasses validation. The proven `build_bulletin → layout::markup → Typst`
//! pipeline is reused wholesale: the AI's job ends the moment it produces specs.
//!
//! ## Module layout — pure core, thin I/O shell (mirrors `layout::engine`)
//!
//! - [`prompt`] — PURE. Builds the Messages API request body (system prompt +
//!   user turn + a tool whose JSON schema is constrained to the existing
//!   block-kind catalogue). No network, no key.
//! - [`parse`] — PURE. Validates and sanitises the model's tool-use response
//!   into ordered `BlockSpec`s, rejecting unknown kinds and malformed payloads.
//!   The LLM only *suggests*; this module *decides* what reaches app state.
//! - [`client`] — the only impure part, behind the `ai` cargo feature. Sends
//!   the request over HTTPS (reqwest) with the key read at call time, then runs
//!   the pure parser on the response. A build without the feature returns a
//!   clear `feature_disabled` error so the UI can show "AI ikke aktivert" and
//!   the manual builder stays unaffected.
//!
//! Because `prompt` and `parse` are pure they carry the real test coverage —
//! unit-tested with canned fixture JSON, no network and no key in the gate.
//!
//! ## Privacy (CLAUDE.md "Privacy is non-negotiable")
//!
//! Every call is **consent-gated** (the caller must pass `consent: true`, which
//! the command derives from the persisted `cloud_ai_enabled` setting) and
//! **purpose-tagged**. The intent text is the ONLY user content sent. Form and
//! member content is never part of an intent→layout request — the request
//! builder takes a plain intent string and nothing else, so there is no channel
//! for form/member data to leak into the prompt.

pub mod client;
pub mod parse;
pub mod prompt;

pub use parse::{parse_block_tree, IntentCompileResult};
pub use prompt::{build_request, IntentRequest, MODEL_ID};
