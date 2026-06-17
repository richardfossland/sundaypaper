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
use crate::services::block::{Block, BlockRepo};
use crate::services::bulletin::{build_bulletin, BlockSpec, ServicePlan};
use crate::services::bulletin_contract::{bulletin_from_contract, ContractServicePlan};
use crate::services::document::{Document, DocumentRepo};
use crate::services::layout::engine;
use crate::services::layout::markup::{build_typst_document, LayoutMeta, RenderBlock};
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

    persist_program(&state, &project_id, doc_title, &specs).await
}

/// Generate a printable program document from a *canonical* SundayPlan service
/// plan handed over as JSON — the wired Plan→Paper bridge.
///
/// The renderer passes the published `sunday-contracts` `ServicePlan` as a JSON
/// string (already fetched from SundayPlan, or pasted by the operator — this
/// command does **no** network fetch, that transport stays out of scope). We
/// deserialise it into [`ContractServicePlan`], run the pure, tested
/// [`bulletin_from_contract`] adapter to get ordered [`BlockSpec`]s, then persist
/// them through the exact same `persist_program` path `bulletin_generate` uses.
///
/// Steps, in order:
/// 1. `serde_json::from_str` → [`ContractServicePlan`] (a malformed plan fails
///    fast as a `json` error before anything is created).
/// 2. `bulletin_from_contract(plan)` — pure mapping to ordered block specs
///    (validates the plan has at least one item).
/// 3. Persist a `program` document plus one top-level block per spec, in order.
///
/// Returns the created `program` document; the renderer lists its blocks via the
/// existing `block_list` command, exactly as with `bulletin_generate`.
#[tauri::command]
pub async fn bulletin_generate_from_plan(
    state: State<'_, AppState>,
    project_id: String,
    plan_json: String,
    title: Option<String>,
) -> AppResult<Document> {
    // Deserialise the canonical contract plan first: a bad paste fails as a
    // `json` error before we touch the database.
    let plan: ContractServicePlan = serde_json::from_str(&plan_json)?;

    // Pure adapter: canonical contract → ordered block specs (rejects an empty
    // plan via build_bulletin) before any I/O.
    let specs = bulletin_from_contract(plan.clone())?;

    // Title: explicit arg → plan's service name → default. Trim so a blank arg
    // doesn't slip past the repo's "title is required" check.
    let doc_title = title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(plan
            .service
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()))
        .unwrap_or("Service Program");

    persist_program(&state, &project_id, doc_title, &specs).await
}

/// Persist ordered [`BlockSpec`]s as a fresh `program` document: create the
/// document in `project_id`, then create one top-level block per spec, in order.
///
/// The single block-insert path shared by `bulletin_generate` (local plan) and
/// `bulletin_generate_from_plan` (canonical contract plan), so neither command
/// duplicates persistence logic. `BlockRepo::create` appends, so insertion order
/// is preserved as each block's `position`.
pub(crate) async fn persist_program(
    state: &AppState,
    project_id: &str,
    title: &str,
    specs: &[BlockSpec],
) -> AppResult<Document> {
    let docs = DocumentRepo::new(state.db.clone());
    let document = docs.create(project_id, title, "program", "A4").await?;

    let blocks = BlockRepo::new(state.db.clone());
    for spec in specs {
        // Top-level blocks (parent_id = None); create() appends so the loop
        // order becomes the printed order.
        blocks
            .create(&document.id, None, &spec.kind, &spec.data)
            .await?;
    }

    Ok(document)
}

/// Render a document's block tree to Typst source — the second half of the
/// FORWARD pipeline (`bulletin_generate` builds the tree, this renders it).
///
/// Steps, in order:
/// 1. Fetch the document record (404 if it's gone or soft-deleted) — its
///    `page_size` seeds the page metadata when the caller doesn't override it.
/// 2. List the document's blocks (flat, position-ordered) and rebuild the tree
///    by grouping on `parent_id`.
/// 3. `build_typst_document(&meta, &blocks)` — pure markup assembly.
///
/// Returns the Typst source string. Compiling it to a PDF is a later, gated step
/// (the `typst` cargo feature); the source here is fully usable / inspectable on
/// its own. `layout_meta` is optional: when omitted we derive a sensible
/// `LayoutMeta` from the document's page size.
#[tauri::command]
pub async fn bulletin_render(
    state: State<'_, AppState>,
    document_id: String,
    layout_meta: Option<LayoutMeta>,
) -> AppResult<String> {
    let document = DocumentRepo::new(state.db.clone())
        .get(&document_id)
        .await?;

    let rows = BlockRepo::new(state.db.clone())
        .list_by_document(&document_id)
        .await?;
    let blocks = build_render_tree(&rows);

    // Caller override wins; otherwise seed the page size from the document and
    // keep the rest of `LayoutMeta`'s defaults.
    let meta = layout_meta.unwrap_or_else(|| LayoutMeta {
        paper: document.page_size.clone(),
        ..LayoutMeta::default()
    });

    Ok(build_typst_document(&meta, &blocks))
}

/// Compile Typst source to a PDF — the final FORWARD-pipeline step.
///
/// Takes the source string `bulletin_render` produces (or any Typst markup) and
/// returns the rendered PDF as a base64 string (no data-URL prefix), mirroring
/// `pdf_render_page`, so the renderer can drop it into a download or an
/// `<embed src="data:application/pdf;base64,...">`.
///
/// Compilation happens in-process via the embedded Typst compiler behind the
/// `typst` cargo feature; a build without it returns a `feature_disabled` error,
/// and invalid source returns a `pdf` error carrying Typst's own diagnostic.
#[tauri::command]
pub async fn typst_compile(_state: State<'_, AppState>, source: String) -> AppResult<String> {
    let bytes = engine::compile(&source)?;
    Ok(base64_encode(&bytes))
}

/// Minimal standard-base64 encoder (no deps) for PDF bytes — same routine as
/// `commands::pdf`, kept local so the two command modules stay independent.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 0x3F) as usize] as char);
        out.push(TABLE[(n >> 12 & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Rebuild the ordered block tree from a flat, position-sorted block list.
///
/// `list_by_document` returns every block in `position` order; here we group the
/// rows by `parent_id` so each node carries its children, then assemble the
/// top-level forest (`parent_id IS NULL`). Children inherit their parent's
/// order because we visit the flat list in its existing `position` order. A row
/// whose `data` doesn't parse degrades to an empty object via
/// [`RenderBlock::from_spec`], so a single bad payload never sinks the render.
fn build_render_tree(rows: &[Block]) -> Vec<RenderBlock> {
    fn children_of(rows: &[Block], parent_id: Option<&str>) -> Vec<RenderBlock> {
        rows.iter()
            .filter(|b| b.parent_id.as_deref() == parent_id)
            .map(|b| {
                let mut node = RenderBlock::from_spec(&b.kind, &b.data);
                node.children = children_of(rows, Some(&b.id));
                node
            })
            .collect()
    }
    children_of(rows, None)
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

    // --- bulletin_generate_from_plan -----------------------------------------
    // The canonical Plan→Paper bridge. The pure mapping (contract → BlockSpec)
    // is exhaustively tested in `services::bulletin_contract`; here we prove the
    // *command's seam*: the golden contract JSON deserialises into
    // `ContractServicePlan`, the adapter runs, and the specs land as ordered
    // blocks through the same persistence path the command uses. The command
    // itself takes `State<'_, AppState>` (not constructible in a unit test), so —
    // like the `bulletin_generate` tests above — we drive the identical sequence
    // (from_str → bulletin_from_contract → create document → create blocks)
    // against a temp in-memory db.

    /// The shared platform golden fixture, the exact JSON a real SundayPlan
    /// hands over. Vendored into this repo's test tree (kept byte-identical to
    /// `sunday-platform/fixtures/service_plan.json`).
    const GOLDEN_PLAN: &str = include_str!("../../tests/fixtures/service_plan.json");

    async fn persist_contract(db: &Db, project_id: &str, plan_json: &str) -> Document {
        // Mirror `bulletin_generate_from_plan` exactly: deserialise the canonical
        // contract JSON, run the pure adapter, then persist via the same insert
        // sequence the command performs.
        let plan: crate::services::bulletin_contract::ContractServicePlan =
            serde_json::from_str(plan_json).unwrap();
        let specs =
            crate::services::bulletin_contract::bulletin_from_contract(plan.clone()).unwrap();
        let doc_title = plan
            .service
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Service Program");
        let docs = DocumentRepo::new(db.clone());
        let document = docs
            .create(project_id, doc_title, "program", "A4")
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

    #[tokio::test]
    async fn from_plan_deserialises_golden_fixture_then_persists_blocks_in_order() {
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist_contract(&db, &pid, GOLDEN_PLAN).await;

        // The service name became the document title (header path).
        assert_eq!(doc.kind, "program");
        assert_eq!(doc.title, "Sunday Morning");

        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        // header + 3 fixture items (welcome → song → scripture).
        assert_eq!(blocks.len(), 4);
        let positions: Vec<i64> = blocks.iter().map(|b| b.position).collect();
        assert_eq!(positions, vec![0, 1, 2, 3]);
        let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, vec!["heading", "liturgy", "song", "scripture"]);

        // Spot-check the song payload survived the adapter + persistence verbatim.
        let song = blocks.iter().find(|b| b.kind == "song").unwrap();
        let data: serde_json::Value = serde_json::from_str(&song.data).unwrap();
        assert_eq!(data["title"], "Amazing Grace");
        assert_eq!(data["songId"], "22222222-2222-2222-2222-222222222222");
        assert_eq!(data["number"], "22025");
    }

    #[tokio::test]
    async fn from_plan_all_blocks_are_top_level() {
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist_contract(&db, &pid, GOLDEN_PLAN).await;
        let blocks = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        assert!(blocks.iter().all(|b| b.parent_id.is_none()));
    }

    #[tokio::test]
    async fn from_plan_rejects_malformed_json_before_touching_db() {
        use crate::services::bulletin_contract::{bulletin_from_contract, ContractServicePlan};

        // A bad paste must fail as a `json` error in the deserialise step (the
        // command's `?`), before any document is created.
        let parsed = serde_json::from_str::<ContractServicePlan>("{ not valid json");
        assert!(parsed.is_err(), "malformed JSON fails the deserialise step");

        // And an empty (but valid) contract plan is rejected by the adapter, so
        // the command bails before persisting anything.
        assert!(
            bulletin_from_contract(ContractServicePlan::default()).is_err(),
            "empty plan → no document"
        );
    }

    // --- bulletin_render -----------------------------------------------------
    // Like the generate tests above, the command takes `State<'_, AppState>`
    // which we can't build in a unit test, so these exercise the same sequence
    // the command performs (fetch document → list blocks → build_render_tree →
    // build_typst_document) directly against a temp in-memory db, plus the pure
    // `build_render_tree` helper in isolation.

    async fn render(db: &Db, document: &Document, meta: Option<LayoutMeta>) -> String {
        let rows = BlockRepo::new(db.clone())
            .list_by_document(&document.id)
            .await
            .unwrap();
        let blocks = build_render_tree(&rows);
        let meta = meta.unwrap_or_else(|| LayoutMeta {
            paper: document.page_size.clone(),
            ..LayoutMeta::default()
        });
        build_typst_document(&meta, &blocks)
    }

    #[tokio::test]
    async fn render_roundtrips_a_generated_plan_into_typst() {
        // End-to-end FORWARD pipeline: ServicePlan → generate → blocks → render.
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &plan()).await;

        let src = render(&db, &doc, None).await;

        // Preamble + helpers are present.
        assert!(src.contains("#set page(paper: \"a4\""));
        assert!(src.contains("#let bp-title"));
        // The leading header becomes a bp-title with subtitle.
        assert!(src.contains("#bp-title([Sunday Worship], sub: [St. Olav's]"));
        // The song carries its hymnal number prefix.
        assert!(src.contains("#bp-heading([N13 097 — Holy, Holy, Holy])"));
        // The scripture reading prints its reference as a byline under the title.
        assert!(src.contains("#bp-heading([First Reading])"));
        assert!(src.contains("#bp-byline([John 3:16])"));
    }

    #[tokio::test]
    async fn render_preserves_block_order_in_the_source() {
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = persist(&db, &pid, &plan()).await;

        let src = render(&db, &doc, None).await;
        // Title before the song before the benediction — printed order matches
        // the setlist order.
        let title = src.find("[Sunday Worship]").unwrap();
        let song = src.find("Holy, Holy, Holy").unwrap();
        let benediction = src.find("[Benediction]").unwrap();
        assert!(title < song && song < benediction);
    }

    #[tokio::test]
    async fn render_of_empty_document_is_preamble_only() {
        // A document with no blocks renders just the preamble — no block markup,
        // and it still compiles (helpers + page setup are all there).
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = DocumentRepo::new(db.clone())
            .create(&pid, "Empty", "program", "A4")
            .await
            .unwrap();

        let src = render(&db, &doc, None).await;
        assert!(src.contains("#set page(paper: \"a4\""));
        assert!(src.contains("#let bp-title"));
        assert!(
            !src.contains("#bp-heading("),
            "no blocks → no section markup"
        );
    }

    #[tokio::test]
    async fn render_uses_document_page_size_when_meta_absent() {
        // A5 document with no override → the page size flows into the preamble.
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = DocumentRepo::new(db.clone())
            .create(&pid, "Sheet", "program", "A5")
            .await
            .unwrap();

        let src = render(&db, &doc, None).await;
        assert!(src.contains("paper: \"a5\""));
    }

    #[tokio::test]
    async fn render_layout_meta_override_wins_over_document() {
        // An explicit LayoutMeta takes precedence and its fields propagate into
        // the preamble (paper, font size, lang).
        let db = Db::connect_memory().await.unwrap();
        let pid = project(&db).await;
        let doc = DocumentRepo::new(db.clone())
            .create(&pid, "Sheet", "program", "A4")
            .await
            .unwrap();

        let meta = LayoutMeta {
            paper: "us-letter".into(),
            font_size_pt: 14.0,
            lang: Some("nb".into()),
            ..LayoutMeta::default()
        };
        let src = render(&db, &doc, Some(meta)).await;
        assert!(src.contains("paper: \"us-letter\""));
        assert!(src.contains("size: 14pt"));
        assert!(src.contains("lang: \"nb\""));
    }

    #[test]
    fn build_render_tree_nests_children_under_their_parent() {
        // Two top-level blocks; the first owns a child. The tree groups by
        // parent_id and keeps the flat position order.
        let now = 0;
        let parent = Block {
            id: "p".into(),
            document_id: "d".into(),
            parent_id: None,
            kind: "liturgy".into(),
            position: 0,
            data: r#"{"title":"Section"}"#.into(),
            created_at: now,
            updated_at: now,
        };
        let child = Block {
            id: "c".into(),
            document_id: "d".into(),
            parent_id: Some("p".into()),
            kind: "text".into(),
            position: 0,
            data: r#"{"text":"child line"}"#.into(),
            created_at: now,
            updated_at: now,
        };
        let sibling = Block {
            id: "s".into(),
            document_id: "d".into(),
            parent_id: None,
            kind: "song".into(),
            position: 1,
            data: r#"{"title":"Hymn"}"#.into(),
            created_at: now,
            updated_at: now,
        };

        let tree = build_render_tree(&[parent, child, sibling]);
        assert_eq!(tree.len(), 2, "two top-level blocks");
        assert_eq!(tree[0].kind, "liturgy");
        assert_eq!(tree[0].children.len(), 1, "parent owns its child");
        assert_eq!(tree[0].children[0].kind, "text");
        assert!(tree[1].children.is_empty(), "sibling has no children");

        // And the child renders after its parent in the source.
        let src = build_typst_document(&LayoutMeta::default(), &tree);
        let p = src.find("#bp-heading([Section])").unwrap();
        let c = src.find("#par[child line]").unwrap();
        assert!(p < c);
    }

    #[test]
    fn base64_matches_known_vectors() {
        // The encoder feeding `typst_compile` — same RFC 4648 vectors the pdf
        // command checks, so the two copies can't silently diverge.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn build_render_tree_tolerates_unparseable_data() {
        // A row with garbage data degrades to an empty object rather than
        // panicking, so one bad block can't sink the whole render.
        let row = Block {
            id: "x".into(),
            document_id: "d".into(),
            parent_id: None,
            kind: "text".into(),
            position: 0,
            data: "not json".into(),
            created_at: 0,
            updated_at: 0,
        };
        let tree = build_render_tree(&[row]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].data, serde_json::json!({}));
    }
}
