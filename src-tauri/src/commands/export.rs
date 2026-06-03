//! Batch export IPC command — render many documents to PDF in one pass (Phase 6).
//!
//! A church produces the same Sunday program in several variants (regular,
//! large-print, upload-ready). `bulletin_render` + `typst_compile` already turn
//! ONE document into a PDF; this command loops that exact chain over a *set* of
//! documents, applying one set of [`ExportOptions`] (paper size + large-print
//! scaling + language) to all of them, and writes each result to a directory.
//!
//! `services::export` is the pure half: it validates the request, derives a
//! per-document [`LayoutMeta`], and sanitises a title into a filename. This
//! command is the thin I/O half: fetch each document + its blocks, render → Typst
//! → PDF (the same render tree the Builder/Editor builds), and write the bytes.

use std::path::Path;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::services::block::{Block, BlockRepo};
use crate::services::document::DocumentRepo;
use crate::services::export::{
    file_name_for, layout_for, validate_request, BatchExportResult, ExportOptions, ExportedFile,
};
use crate::services::layout::engine;
use crate::services::layout::markup::{build_typst_document, RenderBlock};
use crate::AppState;

/// Render a set of documents to PDF files in `out_dir`.
///
/// Steps, in order:
/// 1. `validate_request(&document_ids, &options)` — fail fast on an empty /
///    duplicate selection or out-of-range scaling, before any I/O.
/// 2. Ensure `out_dir` exists (create it, like a "save into this folder" flow).
/// 3. For each document, in request order: fetch the record (404 if gone),
///    list + tree its blocks, `build_typst_document` with the per-document
///    `LayoutMeta` from `layout_for`, `engine::compile` to PDF bytes, and write
///    them to `out_dir/<sanitised-title>.pdf`.
///
/// Returns a [`BatchExportResult`] listing every written file, so the renderer
/// can show what landed and where. Any single failure (missing doc, compile
/// error, write error) aborts the batch with that error — the renderer surfaces
/// it like the Builder's render→compile errors.
#[tauri::command]
pub async fn bulletin_batch_export(
    state: State<'_, AppState>,
    document_ids: Vec<String>,
    options: ExportOptions,
    out_dir: String,
) -> AppResult<BatchExportResult> {
    // Pure validation first — never touch the filesystem on a bad request.
    validate_request(&document_ids, &options)?;

    let dir = Path::new(&out_dir);
    if dir.as_os_str().is_empty() {
        return Err(AppError::Validation("output directory is required".into()));
    }
    std::fs::create_dir_all(dir)?;

    let docs = DocumentRepo::new(state.db.clone());
    let blocks_repo = BlockRepo::new(state.db.clone());

    let mut files = Vec::with_capacity(document_ids.len());
    for document_id in &document_ids {
        // Fetch the document (its page size seeds the layout when options don't
        // pin a paper) and rebuild the render tree exactly as bulletin_render.
        let document = docs.get(document_id).await?;
        let rows = blocks_repo.list_by_document(document_id).await?;
        let blocks = build_render_tree(&rows);

        let meta = layout_for(&document.page_size, &options);
        let source = build_typst_document(&meta, &blocks);
        let pdf_bytes = engine::compile(&source)?;

        let file_name = file_name_for(&document.title, &options);
        let path = dir.join(&file_name);
        std::fs::write(&path, &pdf_bytes)?;

        files.push(ExportedFile {
            document_id: document_id.clone(),
            path: path.to_string_lossy().into_owned(),
            file_name,
        });
    }

    Ok(BatchExportResult {
        directory: dir.to_string_lossy().into_owned(),
        files,
    })
}

/// Rebuild the ordered block tree from a flat, position-sorted block list.
///
/// Identical idiom to `commands::bulletin::build_render_tree`, kept local so the
/// two command modules stay independent (same posture as the duplicated
/// `base64_encode`). `list_by_document` returns rows in `position` order; we
/// group by `parent_id` and assemble the top-level forest, so children inherit
/// their parent's order. A row whose `data` doesn't parse degrades to an empty
/// object via [`RenderBlock::from_spec`].
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
    use crate::services::db::Db;
    use crate::services::document::{Document, DocumentRepo};
    use crate::services::project::ProjectRepo;

    // The command takes `State<'_, AppState>`, which we can't build in a unit
    // test, so — like commands::bulletin's tests — these exercise the same
    // sequence the command performs (fetch doc → list blocks → tree → derive
    // layout → build source) directly against a temp in-memory db. Compiling to
    // PDF + writing files is the `typst`-feature half, covered by the pure
    // service tests + the renderer's E2E mock; here we prove the wiring up to
    // the Typst source is correct and that layout overrides reach the markup.

    async fn seed(db: &Db) -> (String, Document) {
        let pid = ProjectRepo::new(db.clone())
            .create("P", "")
            .await
            .unwrap()
            .id;
        let doc = DocumentRepo::new(db.clone())
            .create(&pid, "Søndag", "program", "a5")
            .await
            .unwrap();
        let blocks = BlockRepo::new(db.clone());
        blocks
            .create(
                &doc.id,
                None,
                "heading",
                r#"{"role":"service-title","title":"Søndag"}"#,
            )
            .await
            .unwrap();
        blocks
            .create(&doc.id, None, "text", r#"{"text":"Velkommen"}"#)
            .await
            .unwrap();
        (pid, doc)
    }

    /// The render tree + source built here matches what bulletin_render emits
    /// for the same document, proving the chain is reused faithfully.
    #[tokio::test]
    async fn builds_source_from_document_tree() {
        let db = Db::connect_memory().await.unwrap();
        let (_pid, doc) = seed(&db).await;

        let rows = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        let blocks = build_render_tree(&rows);
        assert_eq!(blocks.len(), 2);

        let meta = layout_for(&doc.page_size, &ExportOptions::default());
        // Document's own page size flows through when options don't pin paper.
        assert_eq!(meta.paper, "a5");

        let source = build_typst_document(&meta, &blocks);
        assert!(source.contains("paper: \"a5\""));
        assert!(source.contains("Velkommen"));
    }

    /// A large-print option scales the font size in the emitted source, which is
    /// the whole user-visible point of the variant.
    #[tokio::test]
    async fn large_print_option_enlarges_source_font() {
        let db = Db::connect_memory().await.unwrap();
        let (_pid, doc) = seed(&db).await;

        let rows = BlockRepo::new(db.clone())
            .list_by_document(&doc.id)
            .await
            .unwrap();
        let blocks = build_render_tree(&rows);

        let opts = ExportOptions {
            large_print_percent: Some(150),
            ..ExportOptions::default()
        };
        let meta = layout_for(&doc.page_size, &opts);
        let source = build_typst_document(&meta, &blocks);
        // 11pt * 1.5 = 16.5pt.
        assert!(
            source.contains("16.5pt"),
            "expected enlarged 16.5pt text size in source, got: {source}"
        );
    }

    /// An options paper size overrides the document's own.
    #[tokio::test]
    async fn paper_option_overrides_document_page_size() {
        let db = Db::connect_memory().await.unwrap();
        let (_pid, doc) = seed(&db).await;

        let opts = ExportOptions {
            paper: Some("us-letter".into()),
            ..ExportOptions::default()
        };
        let meta = layout_for(&doc.page_size, &opts);
        let source = build_typst_document(&meta, &[]);
        assert!(source.contains("paper: \"us-letter\""));
    }
}
