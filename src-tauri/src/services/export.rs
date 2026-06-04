//! Pure "batch export" planning — the options validation + per-document layout
//! derivation half of Phase 6.
//!
//! Churches routinely produce the SAME service program in several variants on
//! the same Sunday: a regular A4 program, a large-print sheet for the back
//! pews, an upload-ready PDF for the website. The batch export feature renders a
//! *set* of documents through the existing FORWARD chain
//! (`build_typst_document` → `engine::compile`) in one pass, applying one set of
//! [`ExportOptions`] to all of them.
//!
//! This module is **pure** (no DB, no Typst compiler, no I/O): it validates the
//! requested options, turns them into a [`LayoutMeta`] override (paper size +
//! large-print font scaling), and sanitises a document title into a safe
//! filename. The thin I/O loop lives in `commands::export`, mirroring how
//! `services::bulletin` is pure and `commands::bulletin` does the persistence.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::services::layout::markup::LayoutMeta;

/// The base font size (points) a regular export uses when the caller doesn't
/// pin one. Matches `LayoutMeta::default`'s 11pt so a 100% scale reproduces the
/// Builder/Editor preview exactly.
const BASE_FONT_SIZE_PT: f64 = 11.0;

/// Large-print scaling is clamped to this inclusive percent range. 100% is a
/// no-op (regular size); the ceiling keeps the result inside `LayoutMeta`'s own
/// 6–48pt clamp and avoids an unreadable single-word-per-page sheet.
const MIN_SCALE_PERCENT: u32 = 100;
const MAX_SCALE_PERCENT: u32 = 300;

/// Options applied to every document in a batch export. One option set, many
/// documents — the whole point of the feature.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/ExportOptions.ts")]
pub struct ExportOptions {
    /// Paper size keyword (`a4`, `a5`, `us-letter`, …). Empty / `null` keeps
    /// each document's own page size. Normalised by `LayoutMeta`'s preamble, so
    /// an unknown value still compiles (falls back to a4).
    #[serde(default)]
    pub paper: Option<String>,
    /// Large-print font scaling, in percent of the base size. `100` (or `null`)
    /// is regular size; `150` is the typical large-print sheet. Clamped to
    /// 100–300%.
    #[serde(default)]
    pub large_print_percent: Option<u32>,
    /// Optional Typst hyphenation language (`nb`, `en`, …) forwarded to the
    /// document. `null` keeps Typst's default.
    #[serde(default)]
    pub lang: Option<String>,
}

/// The result of a single document's export within a batch — surfaced so the
/// renderer can show per-document success/skip and the on-disk path.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/ExportedFile.ts")]
pub struct ExportedFile {
    /// The source document id this file was rendered from.
    pub document_id: String,
    /// Absolute path of the written PDF.
    pub path: String,
    /// The leaf filename (no directory) — handy for a compact UI list.
    pub file_name: String,
}

/// The summary a batch export returns: where the files went and the list of
/// each written PDF, in request order.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/BatchExportResult.ts")]
pub struct BatchExportResult {
    /// The directory every file was written into.
    pub directory: String,
    /// One entry per successfully exported document, in request order.
    pub files: Vec<ExportedFile>,
}

/// Validate the requested document ids + options before any rendering happens
/// — the same fail-fast posture as `build_bulletin` rejecting an empty plan.
///
/// Rules:
///   - at least one document must be selected,
///   - no duplicate document ids (a duplicate would silently overwrite its own
///     file and confuse the per-document result list),
///   - `large_print_percent`, when given, must fall in 100–300%.
pub fn validate_request(document_ids: &[String], options: &ExportOptions) -> AppResult<()> {
    if document_ids.is_empty() {
        return Err(AppError::Validation(
            "select at least one document to export".into(),
        ));
    }

    // Cheap O(n^2) dedup check — batches are a handful of documents, never huge.
    for (i, id) in document_ids.iter().enumerate() {
        if id.trim().is_empty() {
            return Err(AppError::Validation("document id is empty".into()));
        }
        if document_ids[..i].iter().any(|prev| prev == id) {
            return Err(AppError::Validation(format!(
                "document {id} is selected more than once"
            )));
        }
    }

    if let Some(pct) = options.large_print_percent {
        if !(MIN_SCALE_PERCENT..=MAX_SCALE_PERCENT).contains(&pct) {
            return Err(AppError::Validation(format!(
                "large-print scaling must be {MIN_SCALE_PERCENT}–{MAX_SCALE_PERCENT}%, got {pct}%"
            )));
        }
    }

    Ok(())
}

/// Turn the batch options into a `LayoutMeta` override for one document.
///
/// `doc_page_size` is the document's own page size, used when the caller didn't
/// pin a paper size in the options. The font size is the base 11pt scaled by
/// `large_print_percent` (default 100%); `LayoutMeta`'s preamble clamps the
/// final value into a printable range, so an extreme percent can't break the
/// compile.
pub fn layout_for(doc_page_size: &str, options: &ExportOptions) -> LayoutMeta {
    let paper = options
        .paper
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(doc_page_size)
        .to_string();

    let scale = options.large_print_percent.unwrap_or(MIN_SCALE_PERCENT) as f64 / 100.0;

    let lang = options
        .lang
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    LayoutMeta {
        paper,
        font_size_pt: BASE_FONT_SIZE_PT * scale,
        lang,
    }
}

/// Sanitise a document title into a safe, single-segment PDF filename.
///
/// Keeps letters/digits/space/`-`/`_`, collapses everything else (incl. path
/// separators) to `-`, trims, and falls back to `document` when nothing usable
/// remains. A large-print variant gets a `-storskrift` suffix so the regular
/// and large-print files of the same program don't collide in one directory.
pub fn file_name_for(title: &str, options: &ExportOptions) -> String {
    let mut stem = String::with_capacity(title.len());
    let mut last_dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            stem.push(ch);
            last_dash = false;
        } else if !last_dash {
            stem.push('-');
            last_dash = true;
        }
    }
    let stem = stem.trim().trim_matches('-').trim();
    let stem = if stem.is_empty() { "document" } else { stem };

    let large_print = options
        .large_print_percent
        .map(|p| p > MIN_SCALE_PERCENT)
        .unwrap_or(false);

    if large_print {
        format!("{stem}-storskrift.pdf")
    } else {
        format!("{stem}.pdf")
    }
}

/// Make `candidate` unique against the set of filenames already chosen in this
/// batch, recording the result. Two distinct documents that share a title map to
/// the same sanitised name from [`file_name_for`]; without this they would write
/// to the same path and silently overwrite each other (the id-dedup in
/// [`validate_request`] cannot catch it, since the name comes from the title).
///
/// On collision a numeric suffix is inserted before the extension:
/// `Program.pdf` → `Program-2.pdf` → `Program-3.pdf`, etc. `used` is updated
/// with the returned name so the next call sees it.
pub fn dedup_file_name(candidate: &str, used: &mut std::collections::HashSet<String>) -> String {
    if used.insert(candidate.to_string()) {
        return candidate.to_string();
    }
    // Split off a trailing extension (".pdf") so the suffix lands on the stem.
    let (stem, ext) = match candidate.rsplit_once('.') {
        Some((s, e)) => (s, format!(".{e}")),
        None => (candidate, String::new()),
    };
    let mut n = 2u32;
    loop {
        let alt = format!("{stem}-{n}{ext}");
        if used.insert(alt.clone()) {
            return alt;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn dedup_returns_candidate_when_unused() {
        let mut used = HashSet::new();
        assert_eq!(dedup_file_name("Program.pdf", &mut used), "Program.pdf");
        assert!(used.contains("Program.pdf"));
    }

    #[test]
    fn dedup_suffixes_on_collision() {
        // Two different documents with the same title -> same sanitised filename.
        // They must NOT collide on disk: the second gets a numeric suffix.
        let mut used = HashSet::new();
        let a = dedup_file_name("Program.pdf", &mut used);
        let b = dedup_file_name("Program.pdf", &mut used);
        let c = dedup_file_name("Program.pdf", &mut used);
        assert_eq!(a, "Program.pdf");
        assert_eq!(b, "Program-2.pdf");
        assert_eq!(c, "Program-3.pdf");
        // All three are distinct — no silent overwrite.
        assert_eq!(HashSet::from([a, b, c]).len(), 3);
    }

    #[test]
    fn dedup_two_same_title_docs_get_distinct_files() {
        // End-to-end at the pure layer: same title -> same file_name_for output,
        // but dedup_file_name disambiguates them.
        let opts = ExportOptions::default();
        let mut used = HashSet::new();
        let first = dedup_file_name(&file_name_for("Søndagsgudstjeneste", &opts), &mut used);
        let second = dedup_file_name(&file_name_for("Søndagsgudstjeneste", &opts), &mut used);
        assert_ne!(
            first, second,
            "two documents sharing a title must not write the same file"
        );
        assert!(second.ends_with(".pdf"));
    }

    #[test]
    fn rejects_empty_selection() {
        let err = validate_request(&[], &ExportOptions::default()).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_blank_id() {
        let err = validate_request(&["  ".into()], &ExportOptions::default()).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let ids = vec!["doc-1".to_string(), "doc-1".to_string()];
        let err = validate_request(&ids, &ExportOptions::default()).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn accepts_distinct_ids() {
        let ids = vec!["doc-1".to_string(), "doc-2".to_string()];
        assert!(validate_request(&ids, &ExportOptions::default()).is_ok());
    }

    #[test]
    fn rejects_out_of_range_large_print() {
        let opts = ExportOptions {
            large_print_percent: Some(50),
            ..ExportOptions::default()
        };
        assert!(validate_request(&["doc-1".into()], &opts).is_err());

        let opts = ExportOptions {
            large_print_percent: Some(400),
            ..ExportOptions::default()
        };
        assert!(validate_request(&["doc-1".into()], &opts).is_err());
    }

    #[test]
    fn accepts_in_range_large_print() {
        let opts = ExportOptions {
            large_print_percent: Some(150),
            ..ExportOptions::default()
        };
        assert!(validate_request(&["doc-1".into()], &opts).is_ok());
    }

    #[test]
    fn layout_uses_document_page_size_by_default() {
        let meta = layout_for("a5", &ExportOptions::default());
        assert_eq!(meta.paper, "a5");
        assert_eq!(meta.font_size_pt, BASE_FONT_SIZE_PT);
        assert_eq!(meta.lang, None);
    }

    #[test]
    fn layout_option_paper_overrides_document() {
        let opts = ExportOptions {
            paper: Some("us-letter".into()),
            ..ExportOptions::default()
        };
        let meta = layout_for("a4", &opts);
        assert_eq!(meta.paper, "us-letter");
    }

    #[test]
    fn layout_blank_paper_falls_back_to_document() {
        let opts = ExportOptions {
            paper: Some("   ".into()),
            ..ExportOptions::default()
        };
        let meta = layout_for("a4", &opts);
        assert_eq!(meta.paper, "a4");
    }

    #[test]
    fn layout_scales_font_for_large_print() {
        let opts = ExportOptions {
            large_print_percent: Some(150),
            ..ExportOptions::default()
        };
        let meta = layout_for("a4", &opts);
        assert!((meta.font_size_pt - BASE_FONT_SIZE_PT * 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn layout_carries_lang() {
        let opts = ExportOptions {
            lang: Some("nb".into()),
            ..ExportOptions::default()
        };
        let meta = layout_for("a4", &opts);
        assert_eq!(meta.lang.as_deref(), Some("nb"));
    }

    #[test]
    fn file_name_sanitises_punctuation() {
        // The dot after "1" is not in the keep-set, so it collapses to a dash;
        // letters, digits and spaces survive; the ".pdf" extension is appended.
        let name = file_name_for("Søndag 1. juni", &ExportOptions::default());
        assert_eq!(name, "Søndag 1- juni.pdf");
    }

    #[test]
    fn file_name_replaces_path_separators() {
        let name = file_name_for("a/b\\c", &ExportOptions::default());
        assert!(!name.contains('/'));
        assert!(!name.contains('\\'));
        assert!(name.ends_with(".pdf"));
    }

    #[test]
    fn file_name_falls_back_when_empty() {
        let name = file_name_for("///", &ExportOptions::default());
        assert_eq!(name, "document.pdf");
    }

    #[test]
    fn file_name_large_print_suffix() {
        let opts = ExportOptions {
            large_print_percent: Some(150),
            ..ExportOptions::default()
        };
        assert_eq!(file_name_for("Program", &opts), "Program-storskrift.pdf");
    }

    #[test]
    fn file_name_100_percent_is_not_large_print() {
        let opts = ExportOptions {
            large_print_percent: Some(100),
            ..ExportOptions::default()
        };
        assert_eq!(file_name_for("Program", &opts), "Program.pdf");
    }
}
