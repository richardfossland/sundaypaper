//! PURE request-builder for the intent→layout compiler.
//!
//! Turns a free-text intent into the JSON body of an Anthropic Messages API
//! call that *forces* a single tool-use response whose arguments are a block
//! tree constrained to the existing block-kind catalogue. No network, no key —
//! this is the half that carries the test coverage (`build_request` is asserted
//! against expected JSON shape with no I/O at all).
//!
//! The model is told it may ONLY emit blocks of the kinds the renderer already
//! supports; `tool_choice` is pinned to our one tool so the response is always
//! structured (never prose), and the tool's `input_schema` enumerates the legal
//! kinds so the model can't invent a kind the pipeline can't render. The parser
//! re-validates everything regardless — the schema is a hint to the model, not a
//! trust boundary (see `parse`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// The Anthropic model the compiler targets. Matches the suite-wide current
/// Opus. Kept as a single constant so the client and any future caller agree.
pub const MODEL_ID: &str = "claude-opus-4-8";

/// The Messages API version header value the client sends alongside the body.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Name of the single tool we expose to the model. The model is forced to call
/// exactly this tool (`tool_choice`), so its arguments are our structured tree.
pub const TOOL_NAME: &str = "emit_block_tree";

/// Upper bound on the intent length we'll send. A program intent is a sentence
/// or two; anything past this is almost certainly a paste of unrelated content
/// (and a cost / privacy risk), so we reject it before building a request.
pub const MAX_INTENT_CHARS: usize = 2000;

/// Output-token ceiling for the structured response. A populated program tree is
/// small; this is generous headroom while bounding cost.
pub const MAX_TOKENS: u32 = 4096;

/// The block kinds the renderer (`layout::markup`) and the bulletin generator
/// already produce. The AI is constrained to THIS set both in the prompt and in
/// the tool's JSON-schema `enum`, and the parser rejects anything outside it —
/// so a generated tree always flows through the proven pipeline unchanged.
///
/// Deliberately EXCLUDES the form-field kinds (`form_field` / `checkbox` /
/// `signature`): those collect member data, and the privacy promise keeps AI
/// away from form/member content. A program intent never needs them.
pub const ALLOWED_KINDS: &[&str] = &[
    "heading",
    "song",
    "music",
    "scripture",
    "liturgy",
    "announcement",
    "image",
    "text",
];

/// What the renderer asks for in each block's payload, per kind — distilled from
/// `services::bulletin::build_bulletin` so the model fills the same fields the
/// proven pipeline reads. Purely advisory text in the system prompt; the parser
/// does not require any particular field to be present.
fn kind_field_guide() -> &'static str {
    "\
- heading: { role: \"service-title\" | \"sermon\", title, subtitle?, date?, preacher?, synopsis? } \
— the leading service-title header (title/subtitle/date) and the sermon section header.\n\
- song: { title, number?, author?, leader?, verses?: string[], refrain?, copyright? } \
— a hymn / song. `number` is the hymnal number (e.g. \"N13 097\"); leave verses empty if unknown.\n\
- music: { title, leader?, text? } — instrumental music (prelude/postlude), no lyrics.\n\
- scripture: { title?, reader?, book?, reference?, translation?, text? } — a Bible reading.\n\
- liturgy: { role: \"welcome\"|\"creed\"|\"prayer\"|\"communion\"|\"offering\"|\"benediction\"|\"liturgy\", title, leader?, text? } \
— a spoken liturgical element; pick the most specific role.\n\
- announcement: { title, text? } — a notice.\n\
- image: { title?, caption?, url? } — a banner / poster; only when the intent names one.\n\
- text: { title?, text? } — anything that fits no other kind."
}

/// A consent-gated, purpose-tagged intent compile request. Built by the command
/// from the user's prompt-bar text plus the persisted consent flag; passed to
/// [`build_request`]. The intent string is the ONLY user content that leaves the
/// machine — there is no field for form/member data, by construction.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/IntentRequest.ts")]
pub struct IntentRequest {
    /// The free-text intent, e.g. "lag søndagens program for 1. søndag i advent
    /// med to salmer og dåp".
    pub intent: String,
    /// Explicit cloud-AI consent. The command sets this from the persisted
    /// `cloud_ai_enabled` setting; the builder refuses to produce a request when
    /// it is false, so an intent can never be sent without opt-in.
    pub consent: bool,
    /// Purpose tag carried for auditability / data-minimisation (e.g.
    /// "intent_to_layout"). Sent to no third party — it labels the call locally
    /// and is echoed into the system prompt's purpose line.
    #[serde(default)]
    pub purpose: Option<String>,
    /// Optional UI language hint (e.g. "no", "en") so the model labels generated
    /// blocks in the congregation's language. Defaults to Norwegian.
    #[serde(default)]
    pub lang: Option<String>,
}

/// The purpose tag we default to when the caller doesn't supply one.
pub const DEFAULT_PURPOSE: &str = "intent_to_layout";

/// Build the Anthropic Messages API request body for an intent→layout compile.
///
/// Errors (before any network would happen):
/// - `Validation` if consent is false — the privacy gate, enforced in the pure
///   layer so it can't be bypassed by a caller that forgets to check.
/// - `Validation` if the intent is blank or longer than [`MAX_INTENT_CHARS`].
///
/// The returned `Value` is the exact JSON to POST: `model`, `max_tokens`, a
/// Norwegian-first `system` prompt (constraining output to [`ALLOWED_KINDS`] and
/// excluding form/member content), the single user turn carrying the intent, and
/// one tool (`emit_block_tree`) the model is forced to call.
pub fn build_request(req: &IntentRequest) -> AppResult<Value> {
    // Consent gate first — refuse to build a request the user didn't opt into.
    if !req.consent {
        return Err(AppError::Validation(
            "cloud AI is not enabled (consent required) — AI ikke aktivert".into(),
        ));
    }

    let intent = req.intent.trim();
    if intent.is_empty() {
        return Err(AppError::Validation(
            "intent text is required to compile a program".into(),
        ));
    }
    if intent.chars().count() > MAX_INTENT_CHARS {
        return Err(AppError::Validation(format!(
            "intent is too long (max {MAX_INTENT_CHARS} characters)"
        )));
    }

    let lang = req
        .lang
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("no");
    let purpose = req
        .purpose
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PURPOSE);

    Ok(json!({
        "model": MODEL_ID,
        "max_tokens": MAX_TOKENS,
        // Adaptive thinking: the current Opus surface — let the model decide
        // depth; no fixed budget (which would 400 on this model family).
        "thinking": { "type": "adaptive" },
        "system": system_prompt(lang, purpose),
        "tools": [block_tree_tool()],
        // Force the structured response: the model MUST call our one tool, so we
        // never have to parse free prose into a tree.
        "tool_choice": { "type": "tool", "name": TOOL_NAME },
        "messages": [
            { "role": "user", "content": user_content(intent) }
        ],
    }))
}

/// The Norwegian-first system prompt. Tells the model it is SundayPaper's program
/// compiler, that it may only emit the allowed kinds, the field guide per kind,
/// the church-appropriate tone, and — explicitly — that it must never invent or
/// request personal/member/form data.
fn system_prompt(lang: &str, purpose: &str) -> String {
    format!(
        "Du er programkompilatoren i SundayPaper, et verktøy menigheter bruker for å lage \
trykte gudstjenesteprogram. Oppgave (formål: {purpose}): gjør brukerens fritekst-intensjon \
om til et ordnet TRE AV BLOKKER for et menighetsprogram, og kall verktøyet \"{TOOL_NAME}\" \
med treet.\n\n\
Regler:\n\
1. Du kan KUN bruke disse blokk-typene (kind): {kinds}. Ikke finn på andre typer.\n\
2. Rekkefølgen på blokkene er den trykte rekkefølgen i programmet. Start vanligvis med en \
heading med role \"service-title\" (tittel, evt. menighet som subtitle og dato).\n\
3. Felter per type (alle felter unntatt kind er valgfrie — utelat det du ikke vet, ikke gjett \
salmenummer, forfattere eller bibelvers):\n{fields}\n\
4. Skriv all brukervendt tekst (titler, etiketter) på språket \"{lang}\". Hold tonen \
respektfull og passende for kirke/menighet.\n\
5. PERSONVERN: aldri be om, finn på eller ta med navn på enkeltpersoner ut over de rollene \
brukeren selv nevner (f.eks. \"prest\", \"organist\"), og aldri lag skjema-/medlemsfelter. \
Du får kun intensjonsteksten — ingen medlems- eller skjemadata.\n\
6. Hvis intensjonen er uklar, lag et fornuftig standard-program og la valgfrie felter stå tomme \
heller enn å dikte opp innhold.",
        purpose = purpose,
        TOOL_NAME = TOOL_NAME,
        kinds = ALLOWED_KINDS.join(", "),
        fields = kind_field_guide(),
        lang = lang,
    )
}

/// The single user turn: the intent verbatim, wrapped so the model treats it as
/// the content to compile (not as instructions to follow).
fn user_content(intent: &str) -> String {
    format!(
        "Lag et menighetsprogram fra denne intensjonen og kall verktøyet med blokk-treet:\n\n{intent}"
    )
}

/// The one tool the model is forced to call. Its `input_schema` is a strict JSON
/// schema: an object with a `blocks` array, each item an object whose `kind` is
/// one of [`ALLOWED_KINDS`] and whose `data` is a free-form object payload. The
/// `enum` on `kind` is what stops the model emitting a kind the pipeline can't
/// render; the parser still re-checks it.
fn block_tree_tool() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Emit the ordered block tree for the program. Each block is one \
    top-level section of the printed program, in print order.",
        "input_schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "blocks": {
                    "type": "array",
                    "description": "Top-level program blocks, in printed order.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "kind": {
                                "type": "string",
                                "enum": ALLOWED_KINDS,
                                "description": "The block kind. One of the allowed catalogue kinds."
                            },
                            "data": {
                                "type": "object",
                                "description": "Kind-specific payload (title, text, etc.). \
    Use only the fields described for this kind; omit unknown fields."
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

/// Whether a kind string is in the allowed catalogue. Shared with the parser so
/// the prompt schema and the validation can't drift apart.
pub fn is_allowed_kind(kind: &str) -> bool {
    ALLOWED_KINDS.contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(intent: &str) -> IntentRequest {
        IntentRequest {
            intent: intent.into(),
            consent: true,
            purpose: None,
            lang: None,
        }
    }

    #[test]
    fn refuses_without_consent() {
        let r = IntentRequest {
            consent: false,
            ..req("lag et program")
        };
        let err = build_request(&r).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        // The message carries the user-facing "AI ikke aktivert" phrasing.
        assert!(err.to_string().contains("AI ikke aktivert"));
    }

    #[test]
    fn rejects_blank_intent() {
        let err = build_request(&req("   ")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_overlong_intent() {
        let long = "a".repeat(MAX_INTENT_CHARS + 1);
        let err = build_request(&req(&long)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn builds_well_formed_request_body() {
        let body = build_request(&req("lag søndagens program med to salmer og dåp")).unwrap();
        assert_eq!(body["model"], MODEL_ID);
        assert_eq!(body["max_tokens"], MAX_TOKENS);
        assert_eq!(body["thinking"]["type"], "adaptive");
        // Forced tool call to our one tool.
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tool_choice"]["name"], TOOL_NAME);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "exactly one tool exposed");
        assert_eq!(tools[0]["name"], TOOL_NAME);
        // The intent is carried verbatim in the single user turn.
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        let content = messages[0]["content"].as_str().unwrap();
        assert!(content.contains("to salmer og dåp"), "intent is included");
    }

    #[test]
    fn tool_schema_enumerates_exactly_the_allowed_kinds() {
        let body = build_request(&req("program")).unwrap();
        let kind_enum = body["tools"][0]["input_schema"]["properties"]["blocks"]["items"]
            ["properties"]["kind"]["enum"]
            .as_array()
            .expect("kind enum present");
        let kinds: Vec<&str> = kind_enum.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(kinds, ALLOWED_KINDS, "schema enum matches the catalogue");
        // The form-field kinds must NOT be offered (privacy: no member content).
        for forbidden in ["form_field", "checkbox", "signature"] {
            assert!(
                !kinds.contains(&forbidden),
                "form/member kind {forbidden} must not be offered to the AI"
            );
        }
    }

    #[test]
    fn system_prompt_states_privacy_and_purpose_and_language() {
        let r = IntentRequest {
            purpose: Some("intent_to_layout".into()),
            lang: Some("en".into()),
            ..req("program")
        };
        let body = build_request(&r).unwrap();
        let sys = body["system"].as_str().unwrap();
        // Privacy clause present (no member/form data).
        assert!(sys.to_lowercase().contains("personvern"));
        assert!(sys.contains("skjema"));
        // Purpose tag echoed.
        assert!(sys.contains("intent_to_layout"));
        // Requested language flows into the prompt.
        assert!(sys.contains("\"en\""));
    }

    #[test]
    fn default_purpose_and_language_applied_when_absent() {
        let body = build_request(&req("program")).unwrap();
        let sys = body["system"].as_str().unwrap();
        assert!(sys.contains(DEFAULT_PURPOSE));
        // Norwegian default.
        assert!(sys.contains("\"no\""));
    }

    #[test]
    fn allowed_kinds_are_a_subset_the_renderer_supports() {
        // Guard the catalogue: every allowed kind is one the markup builder /
        // bulletin generator actually emits. (Form kinds intentionally excluded.)
        for kind in ALLOWED_KINDS {
            assert!(is_allowed_kind(kind));
        }
        assert!(!is_allowed_kind("form_field"));
        assert!(!is_allowed_kind("totally_made_up"));
    }
}
