//! PURE response parser for the intent→layout AI compiler (Phase 5.1).
//!
//! The impure half ([`super::client`]) does the HTTP round-trip and hands the
//! raw Anthropic Messages API JSON to [`parse_block_tree`], which validates it
//! into the ordered [`BlockSpec`]s the rest of the pipeline already understands.
//! This is where the "LLM only SUGGESTS; the engine decides" rule is enforced:
//!
//! - We read **only** the `emit_block_tree` tool-use input. Free-text the model
//!   may have emitted is ignored — it can't become a block.
//! - Every block's `kind` is checked against the catalogue
//!   ([`is_known_kind`]); an out-of-catalogue kind is dropped, never rendered.
//! - Every block's `data` must be a JSON object (the `block` repo's contract);
//!   a non-object payload is dropped.
//! - A block carrying no usable content (e.g. an empty `data` for a kind that
//!   needs a title/text) is dropped so the program never gets blank sections.
//!
//! Because it's pure it is exhaustively unit-tested against CANNED fixture
//! JSON — no network, no key, no `ai` feature — which is where the real
//! coverage for this feature lives.

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::services::bulletin::BlockSpec;
use crate::services::ai::prompt::{BLOCK_KINDS, TOOL_NAME};

/// Validate a raw Anthropic Messages API response into ordered [`BlockSpec`]s.
///
/// Steps:
/// 1. Find the `emit_block_tree` tool-use block in `content` (the request
///    forced `tool_choice` to it, so a well-formed response has exactly one).
/// 2. Read its `input.blocks` array.
/// 3. For each entry, keep it only if `kind` is in the catalogue and `data` is a
///    JSON object with at least one usable field; serialise to a [`BlockSpec`].
///
/// Errors (each a `Validation` so the renderer can show a clean message):
/// - the response has no `emit_block_tree` tool-use block, or
/// - the tool input has no `blocks` array, or
/// - every block was rejected (nothing renderable came back).
pub fn parse_block_tree(response: &Value) -> AppResult<Vec<BlockSpec>> {
    let input = tool_input(response)
        .ok_or_else(|| AppError::Validation("AI-svaret manglet et blokk-tre".into()))?;

    let blocks = input
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("AI-svaret hadde ingen blokker".into()))?;

    let mut specs = Vec::with_capacity(blocks.len());
    for block in blocks {
        if let Some(spec) = block_to_spec(block) {
            specs.push(spec);
        }
    }

    if specs.is_empty() {
        return Err(AppError::Validation(
            "AI-svaret inneholdt ingen gyldige blokker".into(),
        ));
    }
    Ok(specs)
}

/// Find the `emit_block_tree` tool-use input object in the response `content`.
/// Returns `None` if there is no such block (so the caller can surface a clear
/// error rather than silently producing an empty program).
fn tool_input(response: &Value) -> Option<&Value> {
    response
        .get("content")?
        .as_array()?
        .iter()
        .find(|block| {
            block.get("type").and_then(Value::as_str) == Some("tool_use")
                && block.get("name").and_then(Value::as_str) == Some(TOOL_NAME)
        })
        .and_then(|block| block.get("input"))
}

/// Convert one model-proposed block into a [`BlockSpec`], or `None` if it should
/// be dropped (unknown kind, non-object data, or no usable content). Dropping
/// rather than erroring means one malformed block can't sink the whole tree.
fn block_to_spec(block: &Value) -> Option<BlockSpec> {
    let kind = block.get("kind").and_then(Value::as_str)?.trim();
    if !is_known_kind(kind) {
        return None;
    }
    let data = block.get("data").filter(|d| d.is_object())?;
    if !has_usable_content(data) {
        return None;
    }
    // The `block` repo stores `data` as a JSON string; serialise it the same way
    // `bulletin::BlockSpec::new` does.
    let data_str = serde_json::to_string(data).ok()?;
    Some(BlockSpec {
        kind: kind.to_string(),
        data: data_str,
    })
}

/// Is this kind one the pipeline renders? Checked against the single source of
/// truth — the prompt's catalogue — so the parser and the tool schema can never
/// drift.
pub fn is_known_kind(kind: &str) -> bool {
    BLOCK_KINDS.iter().any(|(k, _)| *k == kind)
}

/// Does the payload carry at least one non-blank string value somewhere? A block
/// whose `data` is `{}` or only empty strings would render as a blank section,
/// so we drop it. Recurses into arrays (e.g. a song's `verses`) so a song with
/// real verses but no title still counts as usable.
fn has_usable_content(data: &Value) -> bool {
    match data {
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(arr) => arr.iter().any(has_usable_content),
        Value::Object(map) => map.values().any(has_usable_content),
        // Numbers / bools / null carry no printable text on their own.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A canned Anthropic response with one `emit_block_tree` tool-use block.
    fn response_with(blocks: Value) -> Value {
        json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "model": "claude-opus-4-8",
            "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "(ignored prose)" },
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": TOOL_NAME,
                    "input": { "blocks": blocks }
                }
            ]
        })
    }

    fn data(spec: &BlockSpec) -> Value {
        serde_json::from_str(&spec.data).unwrap()
    }

    #[test]
    fn parses_a_realistic_advent_program() {
        // The headline example from the spec: "lag søndagens program for 1.
        // søndag i advent med to salmer og dåp".
        let resp = response_with(json!([
            {
                "kind": "heading",
                "data": {
                    "role": "service-title",
                    "title": "Gudstjeneste 1. søndag i advent",
                    "subtitle": "Domkirken",
                    "date": "30. november 2026"
                }
            },
            { "kind": "liturgy", "data": { "role": "welcome", "title": "Velkommen" } },
            {
                "kind": "song",
                "data": { "title": "Gjør døren høy", "number": "5", "verses": ["Gjør døren høy"] }
            },
            { "kind": "scripture", "data": { "book": "Lukas", "reference": "1:26-38" } },
            {
                "kind": "liturgy",
                "data": { "role": "communion", "title": "Dåp", "text": "Vi døper N.N." }
            },
            { "kind": "song", "data": { "title": "Deg være ære", "number": "197" } },
            { "kind": "liturgy", "data": { "role": "benediction", "title": "Velsignelse" } }
        ]));

        let specs = parse_block_tree(&resp).unwrap();
        let kinds: Vec<&str> = specs.iter().map(|s| s.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["heading", "liturgy", "song", "scripture", "liturgy", "song", "liturgy"]
        );
        // Order is preserved and payloads survive verbatim.
        assert_eq!(data(&specs[0])["role"], "service-title");
        assert_eq!(data(&specs[2])["number"], "5");
        assert_eq!(data(&specs[4])["title"], "Dåp");
    }

    #[test]
    fn every_spec_data_is_valid_json_for_the_block_repo() {
        let resp = response_with(json!([
            { "kind": "text", "data": { "title": "Notes", "text": "hei" } }
        ]));
        let specs = parse_block_tree(&resp).unwrap();
        for spec in &specs {
            serde_json::from_str::<Value>(&spec.data).expect("data is valid JSON");
            assert!(!spec.kind.trim().is_empty());
        }
    }

    #[test]
    fn drops_out_of_catalogue_kinds() {
        let resp = response_with(json!([
            { "kind": "song", "data": { "title": "Ekte" } },
            { "kind": "qr_code", "data": { "url": "https://x" } },
            { "kind": "video", "data": { "title": "nope" } }
        ]));
        let specs = parse_block_tree(&resp).unwrap();
        // Only the known kind survives; the model can't smuggle in a new kind.
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].kind, "song");
    }

    #[test]
    fn drops_blocks_with_non_object_or_empty_data() {
        let resp = response_with(json!([
            { "kind": "text", "data": "a string, not an object" },
            { "kind": "text", "data": {} },
            { "kind": "text", "data": { "title": "   " } },
            { "kind": "text", "data": { "title": "Real" } }
        ]));
        let specs = parse_block_tree(&resp).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(data(&specs[0])["title"], "Real");
    }

    #[test]
    fn keeps_a_song_with_verses_but_no_title() {
        // Usable content can live in a nested array — a song with real verses.
        let resp = response_with(json!([
            { "kind": "song", "data": { "verses": ["Amazing grace"] } }
        ]));
        let specs = parse_block_tree(&resp).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(data(&specs[0])["verses"][0], "Amazing grace");
    }

    #[test]
    fn drops_a_song_whose_verses_are_all_blank() {
        let resp = response_with(json!([
            { "kind": "song", "data": { "verses": ["  ", ""] } }
        ]));
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "nothing usable → error");
    }

    #[test]
    fn errors_when_no_tool_use_block_present() {
        // A response that only contains prose (e.g. the model refused to use the
        // tool) yields a clear validation error, not a panic or empty program.
        let resp = json!({
            "content": [ { "type": "text", "text": "Beklager, jeg kan ikke." } ]
        });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn errors_when_tool_input_has_no_blocks_array() {
        let resp = response_with(json!("not an array"));
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn errors_when_all_blocks_are_rejected() {
        let resp = response_with(json!([
            { "kind": "unknown", "data": { "x": "y" } }
        ]));
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn is_known_kind_matches_the_catalogue() {
        for (kind, _) in BLOCK_KINDS {
            assert!(is_known_kind(kind));
        }
        assert!(!is_known_kind("totally_new"));
        assert!(!is_known_kind(""));
    }

    #[test]
    fn parsed_specs_flow_into_layout_markup() {
        // Prove the end-to-end contract: a parsed spec is renderable by the
        // existing pure builder, so the AI output joins the proven pipeline.
        use crate::services::layout::markup::{build_typst_document, LayoutMeta, RenderBlock};
        let resp = response_with(json!([
            { "kind": "heading", "data": { "role": "service-title", "title": "Advent" } },
            { "kind": "song", "data": { "title": "Salme", "number": "5" } }
        ]));
        let specs = parse_block_tree(&resp).unwrap();
        let blocks: Vec<RenderBlock> = specs
            .iter()
            .map(|s| RenderBlock::from_spec(&s.kind, &s.data))
            .collect();
        let src = build_typst_document(&LayoutMeta::default(), &blocks);
        assert!(src.contains("#bp-title([Advent]"));
        assert!(src.contains("#bp-heading([5 — Salme])"));
    }

    #[test]
    fn ignores_a_second_unexpected_tool_use_block() {
        // Defensive: only the emit_block_tree input is read; a stray tool block
        // of another name is ignored.
        let mut resp = response_with(json!([{ "kind": "text", "data": { "text": "ok" } }]));
        resp["content"].as_array_mut().unwrap().push(json!({
            "type": "tool_use",
            "id": "toolu_2",
            "name": "some_other_tool",
            "input": { "blocks": [{ "kind": "text", "data": { "text": "smuggled" } }] }
        }));
        let specs = parse_block_tree(&resp).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(data(&specs[0])["text"], "ok");
    }
}
