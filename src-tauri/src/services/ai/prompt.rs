//! PURE request builder for the intent→layout AI compiler (Phase 5.1).
//!
//! This is the deterministic half of the AI seam: free-text intent → the exact
//! JSON body sent to the Anthropic Messages API. No network, no key, no I/O —
//! so it is exhaustively unit-tested with no `ai` feature and no secrets, the
//! same posture as `layout::markup` and `pdf::plan`.
//!
//! The design rule that makes the whole feature trustworthy: **Claude only
//! SUGGESTS a tree; the engine decides.** We don't ask the model for free-form
//! markup or Typst. We force it through a single `emit_block_tree` **tool** whose
//! JSON schema enumerates the *exact* block kinds the existing
//! `build_bulletin` → `layout::markup` → Typst pipeline already renders
//! (`heading` / `song` / `scripture` / `liturgy` / `music` / `announcement` /
//! `image` / `text`). The structured tool input is then validated by
//! [`super::parse`] against the same catalogue before a single block touches app
//! state — an out-of-catalogue kind is dropped, never rendered.
//!
//! Privacy (CLAUDE.md, promise #4): the payload carries ONLY the user's free
//! intent plus the church/date context the caller chose to share. Form and
//! member content is excluded by construction — this builder has no access to
//! it and the command never passes it in. The purpose tag (`paper.intent`) is
//! attached as request metadata so the call is auditable.

use serde_json::{json, Value};

/// The current Anthropic model. Kept as a constant so the model id lives in one
/// place (mirrors how the suite's other Anthropic seams pin the model). Opus is
/// the right tier for the structured-reasoning "compile an order of service"
/// task; the caller can't override it, so the model can never be downgraded by
/// untrusted input.
pub const MODEL: &str = "claude-opus-4-8";

/// Max output tokens for the tool call. A populated order-of-service tree is
/// small (a dozen-ish blocks); 8k is comfortable headroom and bounds cost.
pub const MAX_TOKENS: u32 = 8192;

/// The Anthropic API version header value.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// The purpose tag attached to every request's `metadata.user_id`-free metadata
/// so the cloud call is auditable as "intent→layout only" (never form/member
/// work). Surfaced in the consent copy in the UI.
pub const PURPOSE_TAG: &str = "paper.intent";

/// The single tool we expose. Forcing `tool_choice` to this tool guarantees the
/// model answers with a structured block tree and nothing else.
pub const TOOL_NAME: &str = "emit_block_tree";

/// Optional, caller-supplied context shared with the model alongside the intent.
/// Every field is the church's own service metadata — never form/member data.
/// All optional so the caller can share as little as it likes (privacy default:
/// share nothing).
#[derive(Debug, Clone, Default)]
pub struct IntentContext {
    /// Congregation / parish name, e.g. "Domkirken".
    pub church: Option<String>,
    /// Human date string, e.g. "1. søndag i advent" or "30. november 2026".
    pub date: Option<String>,
    /// BCP-47-ish language hint for the generated text (`nb`, `en`, …). Drives
    /// the model's output language; defaults to Norwegian per the product's
    /// "Norwegian-first" rule when absent.
    pub lang: Option<String>,
}

/// The block kinds the pipeline renders, with a one-line description each. This
/// is the contract the model is constrained to — kept in lockstep with the
/// `match` arms in `layout::markup::render_block` and the shapes
/// `bulletin::build_bulletin` emits. The parser ([`super::parse`]) rejects any
/// kind not in this list.
pub const BLOCK_KINDS: &[(&str, &str)] = &[
    (
        "heading",
        "A section heading. For the leading service title use data.role = \
         \"service-title\" with title/subtitle/date; for a sermon use \
         data.role = \"sermon\" with title + optional preacher + synopsis.",
    ),
    (
        "song",
        "A hymn / song. data: title, optional number (hymnal no.), author, \
         verses (array of strings, one per verse), refrain, copyright.",
    ),
    (
        "music",
        "Instrumental music with no congregational part (prelude, postlude). \
         data: title, optional leader, text (a short note).",
    ),
    (
        "scripture",
        "A Bible reading. data: optional title, reader, book, reference (e.g. \
         \"3:16-21\"), translation, text (the read passage).",
    ),
    (
        "liturgy",
        "A spoken/read liturgical element. data: role (one of welcome, creed, \
         prayer, communion, offering, benediction, liturgy), title, optional \
         leader, text.",
    ),
    (
        "announcement",
        "A notice. data: title, text.",
    ),
    (
        "image",
        "A poster / banner. data: optional title, caption, url (a path the \
         user supplied — leave absent if none).",
    ),
    (
        "text",
        "Any other free paragraph. data: optional title, text. The safe \
         fallback when nothing else fits.",
    ),
];

/// Build the system prompt: who the model is, the hard rules, and the block
/// catalogue. Deterministic — the same inputs always produce the same string.
pub fn system_prompt(ctx: &IntentContext) -> String {
    let lang = ctx
        .lang
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("nb");
    let mut catalogue = String::new();
    for (kind, desc) in BLOCK_KINDS {
        catalogue.push_str(&format!("- {kind}: {desc}\n"));
    }

    format!(
        "Du er layout-kompilatoren i SundayPaper, et verktøy menigheter bruker \
         til å lage trykte gudstjenesteprogram. Oppgaven din er å gjøre et fritt \
         formulert ønske om til et STRUKTURERT BLOKK-TRE — ikke pyntet tekst.\n\n\
         Regler:\n\
         1. Svar KUN ved å kalle verktøyet `{TOOL_NAME}`. Ikke skriv prosa.\n\
         2. Bruk utelukkende blokktypene under. Ikke finn på nye typer.\n\
         3. Rekkefølgen på blokkene er rekkefølgen de trykkes i — følg en naturlig \
            gudstjenesteflyt (inngang/velkomst → salmer/lesninger/preken → \
            nattverd/forbønn → utgang/velsignelse).\n\
         4. Skriv all brukervendt tekst på språket «{lang}» (norsk bokmål som \
            standard). Hold tonen kirkelig og verdig.\n\
         5. Du foreslår bare et utkast; det er mennesket som redigerer og \
            godkjenner. Ikke ta med personopplysninger eller skjemadata.\n\n\
         Tilgjengelige blokktyper:\n{catalogue}"
    )
}

/// Build the tool definition (`emit_block_tree`) the request advertises. The
/// schema describes a flat, ordered list of `{kind, data}` blocks — the exact
/// `BlockSpec` shape the rest of the pipeline persists. `data` is left as a free
/// object (Anthropic's schema subset doesn't constrain it further); the parser
/// enforces the per-kind shape.
pub fn tool_definition() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Emit the ordered block tree for the requested church \
            document. Each block is a {kind, data} pair drawn from the fixed \
            SundayPaper block catalogue.",
        "input_schema": {
            "type": "object",
            "properties": {
                "blocks": {
                    "type": "array",
                    "description": "Top-level blocks, in printed order.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {
                                "type": "string",
                                "description": "One of the catalogue block \
                                    kinds (heading, song, music, scripture, \
                                    liturgy, announcement, image, text).",
                                "enum": kind_enum(),
                            },
                            "data": {
                                "type": "object",
                                "description": "Kind-specific payload, per the \
                                    catalogue (e.g. a song carries title / \
                                    number / verses / refrain).",
                            }
                        },
                        "required": ["kind", "data"]
                    }
                }
            },
            "required": ["blocks"]
        }
    })
}

/// The catalogue kinds as a JSON array of strings, for the tool's `enum`.
fn kind_enum() -> Vec<Value> {
    BLOCK_KINDS
        .iter()
        .map(|(kind, _)| Value::String((*kind).to_string()))
        .collect()
}

/// Assemble the complete Anthropic Messages API request body.
///
/// `intent` is the user's free-text request (already trimmed by the caller). The
/// `ctx` carries any church/date/language the caller chose to share — and ONLY
/// that. We force `tool_choice` to `emit_block_tree` so the model can only
/// respond with a structured tree.
pub fn build_request(intent: &str, ctx: &IntentContext) -> Value {
    let user_text = compose_user_message(intent, ctx);
    json!({
        "model": MODEL,
        "max_tokens": MAX_TOKENS,
        "system": system_prompt(ctx),
        "tools": [tool_definition()],
        "tool_choice": { "type": "tool", "name": TOOL_NAME },
        // Purpose tag so the call is auditable as intent→layout only. No
        // member/user identifier is sent — privacy by construction.
        "metadata": { "purpose": PURPOSE_TAG },
        "messages": [
            { "role": "user", "content": user_text }
        ]
    })
}

/// Weave the intent together with any shared church/date context into the single
/// user turn. Blank context fields are skipped so we never send empty lines.
fn compose_user_message(intent: &str, ctx: &IntentContext) -> String {
    let mut lines = Vec::new();
    if let Some(church) = nonblank(&ctx.church) {
        lines.push(format!("Menighet: {church}"));
    }
    if let Some(date) = nonblank(&ctx.date) {
        lines.push(format!("Dato/anledning: {date}"));
    }
    lines.push(format!("Ønske: {}", intent.trim()));
    lines.join("\n")
}

/// Trim + drop-if-blank helper, mirroring `bulletin::opt`.
fn nonblank(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_pins_model_and_forces_the_tool() {
        let req = build_request("lag et enkelt program", &IntentContext::default());
        assert_eq!(req["model"], MODEL);
        assert_eq!(req["tool_choice"]["type"], "tool");
        assert_eq!(req["tool_choice"]["name"], TOOL_NAME);
        assert_eq!(req["max_tokens"], MAX_TOKENS);
        // Exactly one tool is advertised, and it is emit_block_tree.
        let tools = req["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], TOOL_NAME);
    }

    #[test]
    fn request_carries_purpose_tag_and_no_user_identity() {
        let req = build_request("x", &IntentContext::default());
        assert_eq!(req["metadata"]["purpose"], PURPOSE_TAG);
        // The metadata object carries the purpose tag and nothing that could
        // identify a member — privacy by construction.
        let meta = req["metadata"].as_object().unwrap();
        assert_eq!(meta.len(), 1, "only the purpose tag is sent");
        assert!(meta.get("user_id").is_none());
    }

    #[test]
    fn tool_schema_enumerates_exactly_the_catalogue_kinds() {
        let tool = tool_definition();
        let enum_vals = tool["input_schema"]["properties"]["blocks"]["items"]["properties"]
            ["kind"]["enum"]
            .as_array()
            .unwrap();
        let kinds: Vec<&str> = enum_vals.iter().map(|v| v.as_str().unwrap()).collect();
        // Same set, same order as the catalogue — the model can pick nothing else.
        let expected: Vec<&str> = BLOCK_KINDS.iter().map(|(k, _)| *k).collect();
        assert_eq!(kinds, expected);
        // And these are the kinds layout::markup dispatches on.
        for k in [
            "heading",
            "song",
            "music",
            "scripture",
            "liturgy",
            "announcement",
            "image",
            "text",
        ] {
            assert!(kinds.contains(&k), "catalogue is missing {k}");
        }
    }

    #[test]
    fn tool_input_requires_blocks_with_kind_and_data() {
        let tool = tool_definition();
        let items = &tool["input_schema"]["properties"]["blocks"]["items"];
        let required: Vec<&str> = items["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, vec!["kind", "data"]);
        assert_eq!(
            tool["input_schema"]["required"].as_array().unwrap()[0],
            "blocks"
        );
    }

    #[test]
    fn system_prompt_lists_every_block_kind() {
        let sys = system_prompt(&IntentContext::default());
        for (kind, _) in BLOCK_KINDS {
            assert!(sys.contains(kind), "system prompt omits kind {kind}");
        }
        // It names the tool and the "structured tree, not prose" rule.
        assert!(sys.contains(TOOL_NAME));
        assert!(sys.contains("BLOKK-TRE"));
    }

    #[test]
    fn system_prompt_defaults_to_norwegian_bokmaal() {
        let sys = system_prompt(&IntentContext::default());
        assert!(sys.contains("«nb»"), "default language is nb");
        // An explicit language overrides it.
        let en = system_prompt(&IntentContext {
            lang: Some("en".into()),
            ..Default::default()
        });
        assert!(en.contains("«en»"));
    }

    #[test]
    fn user_message_carries_intent_and_shared_context_only() {
        let ctx = IntentContext {
            church: Some("Domkirken".into()),
            date: Some("1. søndag i advent".into()),
            lang: Some("nb".into()),
        };
        let req = build_request(
            "  lag søndagens program med to salmer og dåp  ",
            &ctx,
        );
        let msg = req["messages"][0]["content"].as_str().unwrap();
        assert!(msg.contains("Menighet: Domkirken"));
        assert!(msg.contains("Dato/anledning: 1. søndag i advent"));
        // Intent is trimmed.
        assert!(msg.contains("Ønske: lag søndagens program med to salmer og dåp"));
        assert!(!msg.contains("  lag"), "intent is trimmed");
    }

    #[test]
    fn user_message_omits_blank_context_lines() {
        let ctx = IntentContext {
            church: Some("   ".into()),
            date: None,
            lang: None,
        };
        let req = build_request("noe", &ctx);
        let msg = req["messages"][0]["content"].as_str().unwrap();
        assert!(!msg.contains("Menighet:"), "blank church is dropped");
        assert!(!msg.contains("Dato"), "absent date is dropped");
        assert_eq!(msg, "Ønske: noe");
    }

    #[test]
    fn build_is_deterministic() {
        let ctx = IntentContext {
            church: Some("St. Olav".into()),
            ..Default::default()
        };
        let a = build_request("samme ønske", &ctx);
        let b = build_request("samme ønske", &ctx);
        assert_eq!(a, b, "same inputs → identical request body");
    }
}
