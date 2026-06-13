//! PURE response-parser for the intent→layout compiler.
//!
//! Takes the raw Anthropic Messages API response JSON and turns it into ordered
//! [`BlockSpec`](crate::services::bulletin::BlockSpec)s — the exact shape the
//! existing `bulletin` command persists and renders. This is the trust boundary:
//! the model only *suggests* a tree; this module *decides* what reaches app
//! state. It re-validates everything, never trusting the model to have honoured
//! the tool schema:
//!
//! - finds the forced `tool_use` block named `emit_block_tree`;
//! - surfaces a `refusal` / non-tool stop as a clear error rather than crashing;
//! - drops any block whose `kind` is outside the allowed catalogue (the model
//!   can't smuggle in an un-renderable kind);
//! - coerces each block's `data` to a JSON object (non-objects → `{}`), so every
//!   emitted spec satisfies the block repo's "data must be valid JSON object"
//!   contract;
//! - serialises each `(kind, data)` to a `BlockSpec` exactly like
//!   `build_bulletin`, so the downstream pipeline is byte-for-byte the same.
//!
//! Fully unit-tested with canned fixture JSON — no network, no key.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::services::ai::prompt::{is_allowed_kind, TOOL_NAME};
use crate::services::bulletin::BlockSpec;

/// The result of compiling an intent: the ordered specs plus a little metadata
/// the UI can show ("12 blokker, hvorav 1 ukjent type ble droppet").
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/IntentCompileResult.ts")]
pub struct IntentCompileResult {
    /// The validated, ordered block specs — ready to hand to the same
    /// persistence path `bulletin_generate` uses.
    pub blocks: Vec<BlockSpec>,
    /// How many blocks the model emitted that we dropped because their `kind`
    /// was outside the allowed catalogue. Zero in the happy path; surfaced so
    /// the UI can note a degraded result rather than silently losing content.
    pub dropped_unknown_kinds: u32,
}

/// Parse a Messages API response into validated [`BlockSpec`]s.
///
/// Errors:
/// - `Validation` if the model refused (`stop_reason == "refusal"`).
/// - `Validation` if there is no `emit_block_tree` tool-use block (the model
///   answered with prose instead of the forced tool — shouldn't happen with
///   `tool_choice`, but we don't trust it).
/// - `Validation` if the tool input has no `blocks` array, or it's empty after
///   validation (nothing renderable came back).
pub fn parse_block_tree(response: &Value) -> AppResult<IntentCompileResult> {
    // A safety refusal comes back as a 200 with stop_reason "refusal" — handle
    // it before reading content, which may be empty.
    if response.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err(AppError::Validation(
            "AI declined to generate this program (safety refusal)".into(),
        ));
    }

    let input = tool_use_input(response).ok_or_else(|| {
        AppError::Validation(
            "AI response did not contain the expected block tree (no tool use)".into(),
        )
    })?;

    let raw_blocks = input
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("AI block tree had no `blocks` array".into()))?;

    let mut blocks = Vec::with_capacity(raw_blocks.len());
    let mut dropped_unknown_kinds = 0u32;

    for raw in raw_blocks {
        let Some(kind) = raw.get("kind").and_then(Value::as_str) else {
            // A block with no/!string kind is malformed — drop it, don't crash.
            dropped_unknown_kinds += 1;
            continue;
        };
        let kind = kind.trim();
        if !is_allowed_kind(kind) {
            // The model emitted a kind outside the catalogue. The schema enum
            // should prevent this, but we never trust it — drop and count.
            dropped_unknown_kinds += 1;
            continue;
        }

        // Coerce data to a JSON object; anything else (null, string, array,
        // missing) becomes `{}` so the spec always satisfies the repo contract.
        let data = match raw.get("data") {
            Some(v) if v.is_object() => v.clone(),
            _ => Value::Object(serde_json::Map::new()),
        };

        blocks.push(BlockSpec {
            kind: kind.to_string(),
            // `Value` always serialises; keep `?` so we mirror the repo's
            // "data must be valid JSON" contract end to end.
            data: serde_json::to_string(&data)?,
        });
    }

    if blocks.is_empty() {
        return Err(AppError::Validation(
            "AI returned no usable blocks for this intent".into(),
        ));
    }

    Ok(IntentCompileResult {
        blocks,
        dropped_unknown_kinds,
    })
}

/// Pull the input object of the first `tool_use` content block named
/// [`TOOL_NAME`]. Returns `None` if the response carries no such block (e.g. the
/// model answered with text), so the caller can surface a clear error.
fn tool_use_input(response: &Value) -> Option<&Value> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A realistic Messages API response carrying a forced `emit_block_tree`
    /// tool-use block — the canned fixture the happy-path tests parse.
    fn good_response() -> Value {
        json!({
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "model": "claude-opus-4-8",
            "stop_reason": "tool_use",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": TOOL_NAME,
                    "input": {
                        "blocks": [
                            {
                                "kind": "heading",
                                "data": {
                                    "role": "service-title",
                                    "title": "Gudstjeneste 1. søndag i advent",
                                    "subtitle": "Domkirken"
                                }
                            },
                            {
                                "kind": "song",
                                "data": { "title": "Gjør døren høy", "number": "N13 005" }
                            },
                            {
                                "kind": "liturgy",
                                "data": { "role": "communion", "title": "Dåp" }
                            }
                        ]
                    }
                }
            ]
        })
    }

    fn data(spec: &BlockSpec) -> Value {
        serde_json::from_str(&spec.data).expect("spec data is valid JSON")
    }

    #[test]
    fn parses_canned_tool_use_into_ordered_specs() {
        let result = parse_block_tree(&good_response()).unwrap();
        assert_eq!(result.dropped_unknown_kinds, 0);
        let kinds: Vec<&str> = result.blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, vec!["heading", "song", "liturgy"]);
        // Payload survives verbatim — spot-check the song.
        let song = &result.blocks[1];
        assert_eq!(data(song)["title"], "Gjør døren høy");
        assert_eq!(data(song)["number"], "N13 005");
        // And the order matches the model's array order (printed order).
        assert_eq!(data(&result.blocks[0])["role"], "service-title");
    }

    #[test]
    fn every_emitted_spec_data_is_valid_json_object() {
        // The block repo rejects non-object/non-JSON data; guarantee we never
        // produce any — the whole reason the parser owns coercion.
        let result = parse_block_tree(&good_response()).unwrap();
        for spec in &result.blocks {
            let v: Value = serde_json::from_str(&spec.data).expect("valid JSON");
            assert!(v.is_object(), "spec data is always an object");
            assert!(!spec.kind.trim().is_empty());
        }
    }

    #[test]
    fn drops_unknown_kinds_and_counts_them() {
        let mut resp = good_response();
        let blocks = resp["content"][0]["input"]["blocks"]
            .as_array_mut()
            .unwrap();
        // A kind the renderer can't handle, and a form kind the AI must never use.
        blocks.push(json!({ "kind": "totally_new", "data": { "x": 1 } }));
        blocks.push(json!({ "kind": "signature", "data": { "label": "Sign" } }));

        let result = parse_block_tree(&resp).unwrap();
        // The 3 good blocks survive; the 2 bad ones are dropped + counted.
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.dropped_unknown_kinds, 2);
        assert!(
            result.blocks.iter().all(|b| b.kind != "signature"),
            "form/member kinds never reach app state even if the model emits them"
        );
    }

    #[test]
    fn coerces_non_object_data_to_empty_object() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use", "name": TOOL_NAME, "input": {
                    "blocks": [
                        { "kind": "text", "data": "not an object" },
                        { "kind": "text" }
                    ]
                }
            }]
        });
        let result = parse_block_tree(&resp).unwrap();
        assert_eq!(result.blocks.len(), 2);
        for spec in &result.blocks {
            assert_eq!(data(spec), json!({}), "non-object/missing data → {{}}");
        }
    }

    #[test]
    fn drops_blocks_with_missing_or_non_string_kind() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use", "name": TOOL_NAME, "input": {
                    "blocks": [
                        { "kind": "song", "data": { "title": "Ok" } },
                        { "data": { "title": "no kind" } },
                        { "kind": 42, "data": {} }
                    ]
                }
            }]
        });
        let result = parse_block_tree(&resp).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.dropped_unknown_kinds, 2);
    }

    #[test]
    fn refusal_stop_reason_is_a_clear_error() {
        let resp = json!({ "stop_reason": "refusal", "content": [] });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert!(err.to_string().to_lowercase().contains("refus"));
    }

    #[test]
    fn missing_tool_use_block_is_an_error() {
        // Model answered with prose instead of the forced tool.
        let resp = json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "Sure, here is a program..." }]
        });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn wrong_tool_name_is_not_accepted() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use", "name": "some_other_tool",
                "input": { "blocks": [{ "kind": "text", "data": {} }] }
            }]
        });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn missing_blocks_array_is_an_error() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [{ "type": "tool_use", "name": TOOL_NAME, "input": {} }]
        });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn all_unknown_kinds_yields_no_usable_blocks_error() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use", "name": TOOL_NAME, "input": {
                    "blocks": [{ "kind": "nope", "data": {} }]
                }
            }]
        });
        let err = parse_block_tree(&resp).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert!(err.to_string().contains("no usable blocks"));
    }

    #[test]
    fn finds_tool_use_among_other_content_blocks() {
        // Thinking + text blocks may precede the tool_use; we still find it.
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [
                { "type": "thinking", "thinking": "" },
                { "type": "text", "text": "Her er programmet." },
                { "type": "tool_use", "name": TOOL_NAME, "input": {
                    "blocks": [{ "kind": "heading", "data": { "title": "X" } }]
                }}
            ]
        });
        let result = parse_block_tree(&resp).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].kind, "heading");
    }
}
