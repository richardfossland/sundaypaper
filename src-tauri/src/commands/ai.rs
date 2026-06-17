//! Intent→Layout AI command — the Phase 5 prompt bar wired end to end.
//!
//! `services::ai` is the pure-core + feature-gated-HTTP half (build request →
//! call Anthropic → parse into validated [`BlockSpec`]s). This command is the
//! thin I/O half: it reads the cloud-AI consent flag and the Anthropic key from
//! the local `setting` store, asks the service to compile the intent into a
//! block tree, then persists that tree through the EXACT same `persist_program`
//! path `bulletin_generate` uses — so an AI-compiled program is byte-for-byte a
//! normal program document and renders through the proven pipeline unchanged.
//!
//! Keyless / feature-off behaviour: if the `ai` feature isn't built, or consent
//! is off, or no key is set, the command returns a clear validation /
//! feature-disabled error ("AI ikke aktivert") and creates nothing. The manual
//! builder is entirely unaffected.

use tauri::State;

use crate::commands::bulletin::persist_program;
use crate::error::{AppError, AppResult};
use crate::services::ai::client::compile_intent;
use crate::services::ai::prompt::{IntentRequest, DEFAULT_PURPOSE};
use crate::services::document::Document;
use crate::services::setting::SettingRepo;
use crate::AppState;

/// Local setting keys — kept in sync with the frontend `SETTING_KEYS`
/// (`src/features/settings/settings-keys.ts`).
const KEY_CLOUD_AI_ENABLED: &str = "cloud_ai_enabled";
const KEY_ANTHROPIC_API_KEY: &str = "anthropic_api_key";

/// Compile a free-text intent into a populated `program` document.
///
/// Steps, in order:
/// 1. Read consent (`cloud_ai_enabled`) and the Anthropic key from the local
///    settings store. Consent off → a clear "AI ikke aktivert" validation error
///    (nothing is sent, nothing is created).
/// 2. `compile_intent(intent, key)` — the service builds the Messages API
///    request (pure, consent-gated), calls Anthropic (feature-gated), and parses
///    the response into validated, ordered [`BlockSpec`]s. A build without the
///    `ai` feature, or a missing key, returns `feature_disabled`.
/// 3. Persist a fresh `program` document plus one top-level block per spec, in
///    order, via the shared `persist_program` — identical to `bulletin_generate`.
///
/// Returns the created `program` document; the renderer lists its blocks via the
/// existing `block_list` command, exactly as with the manual builder.
#[tauri::command]
pub async fn ai_compile_intent(
    state: State<'_, AppState>,
    project_id: String,
    intent: String,
    title: Option<String>,
) -> AppResult<Document> {
    let settings = SettingRepo::new(state.db.clone());

    // Consent gate: derived from the persisted opt-in toggle. Off (or unset) →
    // never send anything; surface the same message the UI shows.
    let consent = settings
        .get(KEY_CLOUD_AI_ENABLED)
        .await?
        .as_deref()
        .map(|v| v == "true")
        .unwrap_or(false);
    if !consent {
        return Err(AppError::Validation(
            "AI ikke aktivert — slå på Sky-AI i innstillinger for å bruke intent→program".into(),
        ));
    }

    // Key read at call time. (Today it lives in the local setting store; Phase 8
    // moves it to the OS keychain — same call site, just a different source.)
    let api_key = settings
        .get(KEY_ANTHROPIC_API_KEY)
        .await?
        .unwrap_or_default();

    let req = IntentRequest {
        intent,
        consent,
        purpose: Some(DEFAULT_PURPOSE.to_string()),
        // The document language follows the app locale; default Norwegian.
        lang: settings.get("locale").await?,
    };

    // Build → call → validate. The pure layer rejects a blank/overlong intent
    // before any network; a feature-off or keyless build returns feature_disabled.
    let result = compile_intent(&req, &api_key).await?;

    if result.dropped_unknown_kinds > 0 {
        tracing::warn!(
            dropped = result.dropped_unknown_kinds,
            "AI emitted blocks of unknown kinds; dropped before persisting"
        );
    }

    // Title: explicit arg → default. (We don't parse a title out of the intent;
    // the leading service-title heading the model emits carries the printed one.)
    let doc_title = title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("AI-program");

    persist_program(&state, &project_id, doc_title, &result.blocks).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::block::BlockRepo;
    use crate::services::db::Db;
    use crate::services::project::ProjectRepo;

    // The command takes `State<'_, AppState>`, which can't be built in a unit
    // test, and `compile_intent` needs a key + network. So — like the bulletin
    // command tests — we exercise the command's *seam* directly: the consent
    // read, the keyless behaviour, and that a (canned) compile result persists
    // through the shared `persist_program` path as ordered top-level blocks.
    // The intent→tree mapping itself is exhaustively covered in
    // `services::ai::{prompt,parse}`.

    async fn project(db: &Db) -> String {
        ProjectRepo::new(db.clone())
            .create("P", "")
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn consent_off_by_default_blocks_the_call() {
        // No `cloud_ai_enabled` setting → consent reads false → validation error,
        // mirroring the command's gate (read the same key the command reads).
        let db = Db::connect_memory().await.unwrap();
        let settings = SettingRepo::new(db.clone());
        let consent = settings
            .get(KEY_CLOUD_AI_ENABLED)
            .await
            .unwrap()
            .as_deref()
            .map(|v| v == "true")
            .unwrap_or(false);
        assert!(!consent, "cloud AI is off until explicitly enabled");
    }

    #[tokio::test]
    async fn canned_result_persists_as_ordered_program_blocks() {
        // Prove the persistence seam: a validated compile result lands as a
        // `program` document with one top-level block per spec, in order — the
        // same `persist_program` path the manual builder uses.
        use crate::services::ai::parse::parse_block_tree;
        use serde_json::json;

        let response = json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use",
                "name": crate::services::ai::prompt::TOOL_NAME,
                "input": { "blocks": [
                    { "kind": "heading", "data": { "role": "service-title", "title": "Advent" } },
                    { "kind": "song", "data": { "title": "Gjør døren høy" } },
                    { "kind": "liturgy", "data": { "role": "communion", "title": "Dåp" } }
                ] }
            }]
        });
        let result = parse_block_tree(&response).unwrap();

        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;

        // Drive the same insert sequence persist_program performs (it needs
        // AppState, which we can't construct in a unit test).
        let docs = crate::services::document::DocumentRepo::new(db.clone());
        let document = docs
            .create(&pid, "AI-program", "program", "A4")
            .await
            .unwrap();
        let blocks = BlockRepo::new(db.clone());
        for spec in &result.blocks {
            blocks
                .create(&document.id, None, &spec.kind, &spec.data)
                .await
                .unwrap();
        }

        assert_eq!(document.kind, "program");
        let rows = BlockRepo::new(db.clone())
            .list_by_document(&document.id)
            .await
            .unwrap();
        let kinds: Vec<&str> = rows.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, vec!["heading", "song", "liturgy"]);
        assert!(rows.iter().all(|b| b.parent_id.is_none()));
        let positions: Vec<i64> = rows.iter().map(|b| b.position).collect();
        assert_eq!(positions, vec![0, 1, 2]);
    }
}
