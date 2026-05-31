//! Bulletin IPC command — the SundayPlan → Paper bridge wired end to end.
//!
//! `services::bulletin::build_bulletin` is the pure half (a [`ServicePlan`] →
//! ordered [`BlockSpec`]s). This command is the thin I/O half: it persists those
//! specs as a fresh `program` document plus one top-level block per spec, in
//! order, through the existing `DocumentRepo` / `BlockRepo`. The renderer hands
//! us the plan JSON (mirroring SundayPlan's shape) and gets back the created
//! document so it can open the new program immediately.

use tauri::State;

use crate::error::AppResult;
use crate::services::block::BlockRepo;
use crate::services::bulletin::{build_bulletin, ServicePlan};
use crate::services::document::{Document, DocumentRepo};
use crate::AppState;

/// Generate a printable program document from a planned service.
///
/// Steps, in order:
/// 1. `build_bulletin(&plan)` — pure mapping to ordered block specs (validates
///    the plan has at least one item).
/// 2. Create a `program` document in `project_id` titled `title`, or the plan's
///    own title, or a sensible default.
/// 3. Create one top-level block per spec, in order — `BlockRepo::create`
///    appends, so insertion order is preserved as `position`.
///
/// Returns the created `program` document. The renderer lists its blocks via the
/// existing `block_list` command.
#[tauri::command]
pub async fn bulletin_generate(
    state: State<'_, AppState>,
    project_id: String,
    plan: ServicePlan,
    title: Option<String>,
) -> AppResult<Document> {
    // Pure step first: fail fast on an empty plan before touching the db.
    let specs = build_bulletin(&plan)?;

    // Pick a title: explicit arg → plan title → default. Trim so a blank arg
    // doesn't slip past the repo's "title is required" check.
    let doc_title = title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(plan
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()))
        .unwrap_or("Service Program");

    let docs = DocumentRepo::new(state.db.clone());
    let document = docs.create(&project_id, doc_title, "program", "A4").await?;

    let blocks = BlockRepo::new(state.db.clone());
    for spec in &specs {
        // Top-level blocks (parent_id = None); create() appends so the loop
        // order becomes the printed order.
        blocks
            .create(&document.id, None, &spec.kind, &spec.data)
            .await?;
    }

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::block::BlockRepo;
    use crate::services::bulletin::{ScriptureRef, SetlistItem, SetlistItemKind, SongRef};
    use crate::services::db::Db;
    use crate::services::document::DocumentRepo;
    use crate::services::project::ProjectRepo;

    /// A plan touching enough kinds to prove order + payload survive the round
    /// trip through the repos.
    fn plan() -> ServicePlan {
        ServicePlan {
            title: Some("Sunday Worship".into()),
            church: Some("St. Olav's".into()),
            date: Some("1 June 2026".into()),
            items: vec![
                SetlistItem {
                    kind: SetlistItemKind::Welcome,
                    title: Some("Welcome".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Song,
                    title: Some("Holy, Holy, Holy".into()),
                    song: Some(SongRef {
                        song_id: Some("song-123".into()),
                        tono_work_id: Some("TONO-999".into()),
                        number: Some("N13 097".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Scripture,
                    title: Some("First Reading".into()),
                    scripture: Some(ScriptureRef {
                        book: Some("John".into()),
                        reference: Some("3:16".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Benediction,
                    ..Default::default()
                },
            ],
        }
    }

    // The command takes `State<'_, AppState>`, which we can't build in a unit
    // test. So these tests exercise the same persistence sequence the command
    // performs (build_bulletin → create program document → create blocks in
    // order) directly against a temp in-memory db — the repo pattern used
    // throughout this crate. This guarantees the wiring contract: every spec
    // becomes a block, in order, with its kind + JSON data intact.

    async fn persist(db: &Db, project_id: &str, plan: &ServicePlan) -> Document {
        let specs = build_bulletin(plan).unwrap();
        let docs = DocumentRepo::new(db.clone());
        let document = docs
            .create(project_id, "Service Program", "program", "A4")
            .await
            .unwrap();
        let blocks = BlockRepo::new(db.clone());
        for spec in &specs {
            blocks
                .create(&document.id, None, &spec.kind, &spec.data)
                .await
                .unwrap();
        }
        document
    }

    async fn project(db: &Db) -> String {
        ProjectRepo::new(db.clone())
            .create("P", "")
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn persists_program_document_and_blocks_in_order() {
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &plan()).await;

        assert_eq!(doc.kind, "program");

        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        // header + 4 items.
        assert_eq!(blocks.len(), 5);

        // Order is preserved as the position sequence 0..5.
        let positions: Vec<i64> = blocks.iter().map(|b| b.position).collect();
        assert_eq!(positions, vec![0, 1, 2, 3, 4]);

        let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["heading", "liturgy", "song", "scripture", "liturgy"]
        );

        // The block payloads are the specs' JSON verbatim — spot-check the song.
        let song = blocks.iter().find(|b| b.kind == "song").unwrap();
        let data: serde_json::Value = serde_json::from_str(&song.data).unwrap();
        assert_eq!(data["title"], "Holy, Holy, Holy");
        assert_eq!(data["tonoWorkId"], "TONO-999");
        assert_eq!(data["number"], "N13 097");

        // And the leading header carries the service metadata.
        let head = &blocks[0];
        let hd: serde_json::Value = serde_json::from_str(&head.data).unwrap();
        assert_eq!(hd["role"], "service-title");
        assert_eq!(hd["title"], "Sunday Worship");
        assert_eq!(hd["subtitle"], "St. Olav's");
    }

    #[tokio::test]
    async fn all_blocks_are_top_level() {
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &plan()).await;
        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        assert!(
            blocks.iter().all(|b| b.parent_id.is_none()),
            "bulletin blocks are all top-level program sections"
        );
    }

    #[tokio::test]
    async fn empty_plan_makes_no_document() {
        // build_bulletin rejects an empty plan, so the command bails before
        // creating anything.
        let empty = ServicePlan::default();
        assert!(build_bulletin(&empty).is_err());
    }

    #[tokio::test]
    async fn deserialises_plan_from_canonical_json_then_persists() {
        // Proves the command's `plan: ServicePlan` argument round-trips the JSON
        // a real SundayPlan emits, then lands as ordered blocks.
        let json = r#"{
            "title": "Morgenmesse",
            "church": "Domkirken",
            "items": [
                { "kind": "welcome", "title": "Velkommen" },
                { "kind": "song", "title": "Deg være ære",
                  "song": { "tono_work_id": "T1", "number": "N13 197" } },
                { "kind": "benediction" }
            ]
        }"#;
        let plan: ServicePlan = serde_json::from_str(json).unwrap();

        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &plan).await;

        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        // header + 3 items.
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].kind, "heading");
        assert_eq!(blocks[1].kind, "liturgy"); // welcome
        assert_eq!(blocks[2].kind, "song");
        assert_eq!(blocks[3].kind, "liturgy"); // benediction
    }
}
