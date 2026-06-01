//! PDF split / merge / extract helpers — the "backward" direction API.
//!
//! This module exposes a stable, Path-based API that the commands layer and
//! tests call without needing to know about the internal `services::pdf`
//! module structure. It wraps the existing lopdf/pdfium engine behind clean
//! function signatures and adds `pdf_page_count` — a focused helper used by
//! the ingest UI.
//!
//! All heavy lifting (lopdf manipulation, pdfium rendering) still lives in
//! `services::pdf::{edit, render}`. This file is the *interface contract* for
//! the backward-direction feature: test this, not lopdf directly.
//!
//! The `#[cfg(feature = "pdf")]` guard mirrors `services::pdf::mod` — when the
//! pdf feature is disabled the public stubs still compile and return
//! `FeatureDisabled`, giving the renderer a clear, actionable error.

use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// Return the number of pages in the PDF at `path`.
///
/// This is a focused alternative to `pdf_info` for callers that only need the
/// page count (e.g. to plan a split or validate an operation).
pub fn pdf_page_count(path: &Path) -> AppResult<usize> {
    let info = crate::services::pdf::info(path)?;
    Ok(info.page_count as usize)
}

/// Split `input_path` into one file per page, writing each file as
/// `{stem}_NN.pdf` inside `output_dir`. Returns the paths of all output files
/// in page order.
///
/// Uses the existing `split_every(chunk_size=1)` engine so the lopdf logic is
/// tested in one place.
pub fn split_pdf(input_path: &Path, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");
    let paths = crate::services::pdf::split_every(input_path, 1, output_dir, stem)?;
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

/// Merge the PDFs listed in `inputs` (in order) into a single file at
/// `output_path`. Requires at least two inputs.
pub fn merge_pdfs(inputs: &[&Path], output_path: &Path) -> AppResult<()> {
    let strings: Vec<String> = inputs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    crate::services::pdf::merge(&strings, output_path)
}

/// Write a new PDF at `output` containing only the pages listed in `pages`
/// (1-indexed), drawn from `input`. The pages slice may be in any order;
/// the extracted PDF will contain them in the order given.
///
/// For example, `pages = &[3, 1]` produces a 2-page PDF where the first page
/// is the original page 3 and the second is the original page 1.
pub fn extract_pages(input: &Path, pages: &[usize], output: &Path) -> AppResult<()> {
    if pages.is_empty() {
        return Err(crate::error::AppError::Validation(
            "at least one page must be selected".into(),
        ));
    }
    // Build a comma-separated spec string — e.g. "3,1" — for the existing
    // parse_page_selection engine.
    let spec = pages
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");
    crate::services::pdf::extract_pages(input, &spec, output)
}

// ── Tests ──────────────────────────────────────────────────────────────────
//
// Tests are always compiled (no `#[cfg(feature = "pdf")]` guard here) but
// the functions they call return `FeatureDisabled` when the `pdf` feature is
// absent. We test the *interface contract* — the shapes and error codes —
// rather than lopdf internals. lopdf-level round-trips live in
// `services::pdf::edit::tests`.

#[cfg(test)]
mod tests {
    use super::*;

    // Build a minimal valid N-page PDF using lopdf directly so the tests are
    // self-contained (no fixture files on disk, no external tools).
    #[cfg(feature = "pdf")]
    fn build_test_pdf(path: &Path, n: u32) {
        use lopdf::content::Content;
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..n {
            let content = Content { operations: vec![] };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc.save(path).expect("save test PDF");
    }

    #[cfg(feature = "pdf")]
    fn page_count_of(path: &Path) -> usize {
        lopdf::Document::load(path)
            .expect("load")
            .get_pages()
            .len()
    }

    // ── pdf_page_count ──────────────────────────────────────────────────────

    #[cfg(feature = "pdf")]
    #[test]
    fn page_count_returns_correct_number() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pdf");
        build_test_pdf(&src, 7);
        assert_eq!(pdf_page_count(&src).unwrap(), 7);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn page_count_single_page() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("one.pdf");
        build_test_pdf(&src, 1);
        assert_eq!(pdf_page_count(&src).unwrap(), 1);
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn page_count_returns_feature_disabled_without_pdf_feature() {
        use crate::error::AppError;
        let err = pdf_page_count(Path::new("/nonexistent.pdf")).unwrap_err();
        assert!(matches!(err, AppError::FeatureDisabled { .. }));
    }

    // ── split_pdf ───────────────────────────────────────────────────────────

    #[cfg(feature = "pdf")]
    #[test]
    fn split_produces_one_file_per_page() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("multi.pdf");
        build_test_pdf(&src, 4);
        let out_dir = dir.path().join("split");
        std::fs::create_dir_all(&out_dir).unwrap();
        let parts = split_pdf(&src, &out_dir).unwrap();
        assert_eq!(parts.len(), 4);
        for p in &parts {
            assert!(p.exists(), "output file missing: {}", p.display());
            assert_eq!(page_count_of(p), 1);
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn split_single_page_pdf_yields_one_part() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("single.pdf");
        build_test_pdf(&src, 1);
        let out_dir = dir.path().join("split");
        std::fs::create_dir_all(&out_dir).unwrap();
        let parts = split_pdf(&src, &out_dir).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(page_count_of(&parts[0]), 1);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn split_output_files_are_in_output_dir() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("doc.pdf");
        build_test_pdf(&src, 3);
        let out_dir = dir.path().join("pages");
        std::fs::create_dir_all(&out_dir).unwrap();
        let parts = split_pdf(&src, &out_dir).unwrap();
        for p in &parts {
            assert_eq!(p.parent().unwrap(), out_dir.as_path());
        }
    }

    // ── merge_pdfs ──────────────────────────────────────────────────────────

    #[cfg(feature = "pdf")]
    #[test]
    fn merge_two_pdfs_sums_pages() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        let out = dir.path().join("merged.pdf");
        build_test_pdf(&a, 2);
        build_test_pdf(&b, 3);
        merge_pdfs(&[a.as_path(), b.as_path()], &out).unwrap();
        assert_eq!(page_count_of(&out), 5);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn merge_three_pdfs_sums_all_pages() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        let c = dir.path().join("c.pdf");
        let out = dir.path().join("merged.pdf");
        build_test_pdf(&a, 1);
        build_test_pdf(&b, 2);
        build_test_pdf(&c, 4);
        merge_pdfs(&[a.as_path(), b.as_path(), c.as_path()], &out).unwrap();
        assert_eq!(page_count_of(&out), 7);
    }

    #[test]
    fn merge_single_input_returns_validation_error() {
        use crate::error::AppError;
        // This exercises the `merge` validation path — it errors before trying
        // to open any file, so it works regardless of the `pdf` feature.
        #[cfg(feature = "pdf")]
        {
            let dir = tempfile::tempdir().unwrap();
            let a = dir.path().join("a.pdf");
            build_test_pdf(&a, 1);
            let out = dir.path().join("out.pdf");
            let err = merge_pdfs(&[a.as_path()], &out).unwrap_err();
            assert!(matches!(err, AppError::Validation(_)));
        }
        #[cfg(not(feature = "pdf"))]
        {
            let err =
                merge_pdfs(&[Path::new("/a.pdf")], Path::new("/out.pdf")).unwrap_err();
            assert!(matches!(err, AppError::FeatureDisabled { .. }));
        }
    }

    // ── extract_pages ───────────────────────────────────────────────────────

    #[cfg(feature = "pdf")]
    #[test]
    fn extract_subset_of_pages() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pdf");
        let out = dir.path().join("out.pdf");
        build_test_pdf(&src, 5);
        extract_pages(&src, &[1, 3, 5], &out).unwrap();
        assert_eq!(page_count_of(&out), 3);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn extract_single_page() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pdf");
        let out = dir.path().join("out.pdf");
        build_test_pdf(&src, 6);
        extract_pages(&src, &[4], &out).unwrap();
        assert_eq!(page_count_of(&out), 1);
    }

    #[test]
    fn extract_empty_selection_returns_validation_error() {
        use crate::error::AppError;
        // No file access needed: the empty-slice guard fires first.
        let err =
            extract_pages(Path::new("/any.pdf"), &[], Path::new("/out.pdf")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn extract_out_of_range_page_returns_validation_error() {
        use crate::error::AppError;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pdf");
        let out = dir.path().join("out.pdf");
        build_test_pdf(&src, 3);
        let err = extract_pages(&src, &[99], &out).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
