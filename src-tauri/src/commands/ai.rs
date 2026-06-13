//! AI intent→layout IPC command (Phase 5.1) — the headline feature wired end
//! to end, behind consent + a key.
//!
//! `ai_compile_intent` turns a free-text intent into a fresh `program` document
//! of blocks. It is the AI counterpart to `bulletin_generate`: where that takes
//! a structured `ServicePlan`, this takes plain text, asks Claude for a
//! structured block tree (constrained to the existing catalogue via tool-use),
//! validates it, and persists it through the **exact same** path. The whole
//! FORWARD pipeline (`layout::markup` → Typst) is reused unchanged — the AI only
//! emits the tree.
//!
//! Guards, in order (each fails fast before any cloud call):
//! 1. **Consent** — the `cloud_ai_enabled` privacy toggle must be `"true"`.
//!    Default-off; without it we return a clear "AI ikke aktivert" error and the
//!    manual builder is untouched.
//! 2. **Feature/key** — the `ai` cargo feature must be built in AND an Anthropic
//!    key must be present in the local `setting` store. No key → "AI ikke
//!    aktivert"; no feature → the client stub returns `feature_disabled`.
//! 3. **Pure validation** — `ai::parse` rejects anything out of catalogue before
//!    a block touches the database.
//!
//! Privacy (CLAUDE.md promise #4): the payload carries only the intent plus the
//! optional church/date/lang the caller passes — form and member content is
//! excluded by construction (this command has no access to it).

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::services::ai::client::compile_intent;
use crate::services::ai::prompt::IntentContext;
use crate::services::block::BlockRepo;
use crate::services::bulletin::BlockSpec;
use crate::services::document::{Document, DocumentRepo};
use crate::services::setting::SettingRepo;
use crate::AppState;

/// Setting key holding the user's opt-in to cloud AI. Mirrors
/// `SETTING_KEYS.cloudAiEnabled` in the renderer (`settings-keys.ts`).
const CLOUD_AI_ENABLED_KEY: &str = "cloud_ai_enabled";
/// Setting key holding the Anthropic API key. Mirrors
/// `SETTING_KEYS.anthropicApiKey`. (Keychain storage is Phase 8; until then the
/// key lives in the local setting store, as the Settings page already does.)
const ANTHROPIC_API_KEY_KEY: &str = "anthropic_api_key";

/// Compile a free-text intent into a printable `program` document.
///
/// Steps:
/// 1. Verify consent (`cloud_ai_enabled == "true"`) and read the API key from
///    the local `setting` store — either missing → `validation` "AI ikke
///    aktivert" / "Sky-AI er ikke slått på".
/// 2. `compile_intent(key, intent, ctx)` — build the tool-use request, call
///    Claude, validate the response into ordered [`BlockSpec`]s. (Without the
///    `ai` feature this returns `feature_disabled`.)
/// 3. Persist a fresh `program` document plus one top-level block per spec, in
///    order — the same path `bulletin_generate` uses.
///
/// `intent` is required (non-blank). `title` / `church` / `date` / `lang` are
/// optional context the operator chose to share. Returns the created document;
/// the renderer lists its blocks via `block_list`, exactly as for the manual and
/// plan-import flows.
#[tauri::command]
pub async fn ai_compile_intent(
    state: State<'_, AppState>,
    project_id: String,
    intent: String,
    title: Option<String>,
    church: Option<String>,
    date: Option<String>,
    lang: Option<String>,
) -> AppResult<Document> {
    let intent = intent.trim();
    if intent.is_empty() {
        return Err(AppError::Validation("skriv inn et ønske først".into()));
    }

    let settings = SettingRepo::new(state.db.clone());

    // 1a. Consent gate — cloud AI is opt-in (default off).
    let consented = settings
        .get(CLOUD_AI_ENABLED_KEY)
        .await?
        .as_deref()
        .map(str::trim)
        == Some("true");
    if !consented {
        return Err(AppError::Validation(
            "Sky-AI er ikke slått på. Skru på «Sky-AI (Claude)» i Innstillinger \
             for å bruke intent→layout."
                .into(),
        ));
    }

    // 1b. Key gate — without a key there is nothing to authenticate with.
    let api_key = settings
        .get(ANTHROPIC_API_KEY_KEY)
        .await?
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "AI ikke aktivert: legg inn en Anthropic API-nøkkel i Innstillinger.".into(),
            )
        })?;

    // 2. The cloud call (pure-built request → Claude → pure-validated specs).
    //    Without the `ai` feature this returns feature_disabled ("AI ikke
    //    aktivert"); the manual builder is unaffected.
    let ctx = IntentContext { church, date, lang };
    let specs = compile_intent(&api_key, intent, &ctx).await?;

    // 3. Persist exactly as the bulletin generator does.
    let doc_title = title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("AI-program");

    persist_program(&state, &project_id, doc_title, &specs).await
}

/// Persist ordered [`BlockSpec`]s as a fresh `program` document — the same
/// sequence `commands::bulletin::persist_program` performs (create the document,
/// then one top-level block per spec, in order). Kept local rather than shared
/// so the two command modules stay independent, matching the crate's pattern.
async fn persist_program(
    state: &AppState,
    project_id: &str,
    title: &str,
    specs: &[BlockSpec],
) -> AppResult<Document> {
    let docs = DocumentRepo::new(state.db.clone());
    let document = docs.create(project_id, title, "program", "A4").await?;

    let blocks = BlockRepo::new(state.db.clone());
    for spec in specs {
        blocks
            .create(&document.id, None, &spec.kind, &spec.data)
            .await?;
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::parse::parse_block_tree;
    use crate::services::db::Db;
    use crate::services::project::ProjectRepo;
    use serde_json::json;

    async fn project(db: &Db) -> String {
        ProjectRepo::new(db.clone())
            .create("P", "")
            .await
            .unwrap()
            .id
    }

    // The command takes `State<'_, AppState>`, which we can't construct in a
    // unit test, so — like the bulletin command tests — we exercise the same
    // persistence sequence (parse canned AI response → create document → create
    // blocks in order) directly against a temp in-memory db. This pins the
    // wiring contract: validated AI specs become ordered program blocks.

    fn canned_response() -> serde_json::Value {
        json!({
            "content": [{
                "type": "tool_use",
                "name": crate::services::ai::prompt::TOOL_NAME,
                "input": { "blocks": [
                    { "kind": "heading", "data": { "role": "service-title", "title": "Advent" } },
                    { "kind": "song", "data": { "title": "Gjør døren høy", "number": "5" } },
                    { "kind": "liturgy", "data": { "role": "benediction", "title": "Velsignelse" } }
                ] }
            }]
        })
    }

    async fn persist(db: &Db, project_id: &str, specs: &[BlockSpec]) -> Document {
        let docs = DocumentRepo::new(db.clone());
        let document = docs
            .create(project_id, "AI-program", "program", "A4")
            .await
            .unwrap();
        let blocks = BlockRepo::new(db.clone());
        for spec in specs {
            blocks
                .create(&document.id, None, &spec.kind, &spec.data)
                .await
                .unwrap();
        }
        document
    }

    #[tokio::test]
    async fn ai_specs_persist_as_ordered_program_blocks() {
        let specs = parse_block_tree(&canned_response()).unwrap();

        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &specs).await;

        assert_eq!(doc.kind, "program");

        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, vec!["heading", "song", "liturgy"]);
        let positions: Vec<i64> = blocks.iter().map(|b| b.position).collect();
        assert_eq!(positions, vec![0, 1, 2]);
        assert!(blocks.iter().all(|b| b.parent_id.is_none()));

        // The song payload survived verbatim into the block row.
        let song = blocks.iter().find(|b| b.kind == "song").unwrap();
        let d: serde_json::Value = serde_json::from_str(&song.data).unwrap();
        assert_eq!(d["number"], "5");
    }

    // Consent / key gating is plain setting-store reads. Prove the gate logic
    // against a real repo so the "default off, opt-in" contract is pinned —
    // without needing the (un-constructible) command State.
    #[tokio::test]
    async fn consent_is_off_by_default_and_flips_to_true() {
        let db = Db::connect_memory().await.unwrap();
        let settings = SettingRepo::new(db.clone());

        // Absent → not consented.
        let consented = settings
            .get(CLOUD_AI_ENABLED_KEY)
            .await
            .unwrap()
            .as_deref()
            == Some("true");
        assert!(!consented, "cloud AI is off by default");

        // Opt in.
        settings.set(CLOUD_AI_ENABLED_KEY, "true").await.unwrap();
        let consented = settings
            .get(CLOUD_AI_ENABLED_KEY)
            .await
            .unwrap()
            .as_deref()
            == Some("true");
        assert!(consented);
    }

    #[tokio::test]
    async fn key_lookup_treats_blank_as_absent() {
        let db = Db::connect_memory().await.unwrap();
        let settings = SettingRepo::new(db.clone());
        settings.set(ANTHROPIC_API_KEY_KEY, "   ").await.unwrap();
        let key = settings
            .get(ANTHROPIC_API_KEY_KEY)
            .await
            .unwrap()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty());
        assert!(key.is_none(), "a blank key is treated as no key");
    }
}
