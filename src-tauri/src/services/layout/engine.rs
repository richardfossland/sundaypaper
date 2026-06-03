//! Typst compilation backend — the final FORWARD-pipeline step (Phase 4.2).
//!
//! `markup::build_typst_document` produces a Typst **source string**; this
//! module compiles that string to **PDF bytes** entirely in memory. Compiling
//! needs the embedded Typst compiler, its standard library, and at least one
//! font, so the whole module sits behind the `typst` cargo feature — the same
//! posture as the `pdf` layer (lopdf/pdfium). When the feature is off, the
//! always-available [`compile`] stub returns a clear `feature_disabled` error so
//! the renderer can tell the user this build can't produce PDFs.
//!
//! The enabled path is self-contained: fonts come bundled via `typst-assets`
//! (the `fonts` feature), and the source is fed in as a single detached file, so
//! compilation touches no system fonts and reads no external files. That keeps
//! the unit tests below fully in-memory and deterministic.

#[cfg(feature = "typst")]
pub use enabled::compile;

#[cfg(not(feature = "typst"))]
pub use disabled::compile;

/// The feature-on implementation: a minimal [`typst::World`] over an in-memory
/// source string + bundled fonts, then `typst::compile` → `typst_pdf::pdf`.
#[cfg(feature = "typst")]
mod enabled {
    use typst::diag::{FileError, FileResult, SourceDiagnostic};
    use typst::foundations::{Bytes, Datetime};
    use typst::layout::PagedDocument;
    use typst::syntax::{FileId, Source};
    use typst::text::{Font, FontBook};
    use typst::utils::LazyHash;
    use typst::{Library, LibraryExt, World};
    use typst_pdf::PdfOptions;

    use crate::error::{AppError, AppResult};

    /// Compile a Typst source string to PDF bytes, fully in memory.
    ///
    /// The source is wrapped in a throwaway [`PaperWorld`] whose only inputs are
    /// the string itself and the fonts bundled by `typst-assets`; nothing is
    /// read from disk. A syntax/compile error (or a PDF-export error) is mapped
    /// to a `Pdf` [`AppError`] carrying Typst's own diagnostic message, so the
    /// renderer can surface exactly what went wrong.
    pub fn compile(source: &str) -> AppResult<Vec<u8>> {
        let world = PaperWorld::new(source);

        // `compile` always returns warnings alongside the result; we only fail
        // on hard errors and otherwise ignore warnings (e.g. an unused font).
        let document: PagedDocument = typst::compile(&world)
            .output
            .map_err(|diags| AppError::Pdf(format_diagnostics("compile", &diags)))?;

        typst_pdf::pdf(&document, &PdfOptions::default())
            .map_err(|diags| AppError::Pdf(format_diagnostics("pdf export", &diags)))
    }

    /// Join Typst diagnostics into one readable, single-line-per-issue message.
    /// Typst reports a list (e.g. several parse errors); we surface the lot so a
    /// bad template isn't hidden behind only its first complaint.
    fn format_diagnostics(stage: &str, diags: &[SourceDiagnostic]) -> String {
        if diags.is_empty() {
            return format!("Typst {stage} failed");
        }
        let joined = diags
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        format!("Typst {stage} error: {joined}")
    }

    /// A throwaway [`World`] holding exactly one in-memory source file plus the
    /// bundled fonts. Built fresh per [`compile`] call — compilation is one-shot
    /// here, so there's no need to share or cache it across calls.
    struct PaperWorld {
        library: LazyHash<Library>,
        book: LazyHash<FontBook>,
        fonts: Vec<Font>,
        main: Source,
    }

    impl PaperWorld {
        fn new(source: &str) -> Self {
            // `Source::detached` mints its own FileId, which we hand back from
            // `main()`; the source is never written to or read from disk.
            let main = Source::detached(source);

            // Load every bundled font face (`typst-assets`'s `fonts` feature).
            // `Font::iter` expands a font file that may carry several faces.
            let fonts: Vec<Font> = typst_assets::fonts()
                .flat_map(|data| Font::iter(Bytes::new(data)))
                .collect();
            let book = FontBook::from_fonts(&fonts);

            Self {
                library: LazyHash::new(Library::builder().build()),
                book: LazyHash::new(book),
                fonts,
                main,
            }
        }
    }

    impl World for PaperWorld {
        fn library(&self) -> &LazyHash<Library> {
            &self.library
        }

        fn book(&self) -> &LazyHash<FontBook> {
            &self.book
        }

        fn main(&self) -> FileId {
            self.main.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            // The document is a single in-memory file; any other id is a bug in
            // the source (e.g. an `include` we don't support) rather than a real
            // file, so report it as not-found instead of touching the disk.
            if id == self.main.id() {
                Ok(self.main.clone())
            } else {
                Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
            }
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            // No external binary files (images/data) are bundled with a compile;
            // the markup builder only emits `image(...)` for assets that exist on
            // disk, and those aren't part of this in-memory pipeline yet.
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }

        fn font(&self, index: usize) -> Option<Font> {
            self.fonts.get(index).cloned()
        }

        fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
            // Deterministic by design: no clock access, so the same source always
            // compiles to the same bytes (Typst's `datetime` errors if used).
            None
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::services::layout::markup::{build_typst_document, LayoutMeta, RenderBlock};
        use serde_json::json;

        /// The standard PDF header. Every PDF file starts with `%PDF-`.
        const PDF_MAGIC: &[u8] = b"%PDF-";

        fn assert_is_pdf(bytes: &[u8]) {
            assert!(!bytes.is_empty(), "PDF output must be non-empty");
            assert!(
                bytes.starts_with(PDF_MAGIC),
                "output must start with the %PDF- magic header"
            );
            // A well-formed PDF ends with the `%%EOF` marker (possibly followed
            // by a trailing newline), so scan the tail rather than the exact end.
            let tail = &bytes[bytes.len().saturating_sub(32)..];
            assert!(
                tail.windows(5).any(|w| w == b"%%EOF"),
                "output must contain the %%EOF trailer"
            );
        }

        /// Compile a real generated program (the pure builder + the compiler),
        /// proving the two halves of the FORWARD pipeline join end to end.
        fn compile_blocks(blocks: &[RenderBlock]) -> Vec<u8> {
            let source = build_typst_document(&LayoutMeta::default(), blocks);
            compile(&source).expect("generated source should compile")
        }

        #[test]
        fn minimal_source_compiles_to_a_valid_pdf() {
            let bytes = compile("Hello, Sunday.").expect("plain text compiles");
            assert_is_pdf(&bytes);
        }

        #[test]
        fn empty_source_compiles_to_a_valid_pdf() {
            // An empty document is legal Typst — it yields a single blank page.
            let bytes = compile("").expect("empty source compiles");
            assert_is_pdf(&bytes);
        }

        #[test]
        fn generated_preamble_only_document_compiles() {
            // The exact output of `build_typst_document` with no blocks: page +
            // text setup and the helper definitions, nothing rendered.
            let source = build_typst_document(&LayoutMeta::default(), &[]);
            let bytes = compile(&source).expect("preamble compiles");
            assert_is_pdf(&bytes);
        }

        #[test]
        fn full_generated_program_compiles() {
            // A representative program touching several block kinds.
            let blocks = vec![
                RenderBlock::leaf(
                    "heading",
                    json!({
                        "role": "service-title",
                        "title": "Sunday Worship",
                        "subtitle": "St. Olav's",
                        "date": "1 June 2026",
                    }),
                ),
                RenderBlock::leaf(
                    "song",
                    json!({ "title": "Holy, Holy, Holy", "number": "N13 097" }),
                ),
                RenderBlock::leaf(
                    "scripture",
                    json!({ "book": "John", "reference": "3:16", "text": "For God so loved the world." }),
                ),
                RenderBlock::leaf("liturgy", json!({ "title": "Benediction" })),
            ];
            assert_is_pdf(&compile_blocks(&blocks));
        }

        #[test]
        fn invalid_typst_syntax_returns_a_helpful_compile_error() {
            // `#let` with no binding is a parse error; the message should name it
            // rather than panic or yield bytes.
            let err = compile("#let").expect_err("broken syntax must fail");
            let msg = err.to_string();
            assert!(
                msg.contains("Typst") && msg.contains("error"),
                "error should mention Typst and the failed stage: {msg}"
            );
            assert!(
                msg.len() > "Typst compile error: ".len(),
                "message carries a diagnostic"
            );
        }

        #[test]
        fn unclosed_content_block_is_reported_not_panicked() {
            // A dangling `[` is unbalanced markup — Typst should report it.
            let err = compile("#box[unterminated").expect_err("unbalanced markup must fail");
            assert!(err.to_string().contains("Typst"), "mapped to a Typst error");
        }

        #[test]
        fn unicode_content_is_preserved_and_compiles() {
            // Nordic letters + an em dash + a non-Latin script: the bundled fonts
            // cover Latin/Nordic; the point is that unicode input compiles
            // cleanly without an encoding panic.
            let blocks = vec![RenderBlock::leaf(
                "heading",
                json!({ "title": "Søndagsmesse — Måløy menighet" }),
            )];
            assert_is_pdf(&compile_blocks(&blocks));
        }

        #[test]
        fn special_chars_in_titles_do_not_break_compilation() {
            // The markup builder escapes these; the compiler must accept the
            // escaped form. A title full of Typst metacharacters used to be the
            // classic injection/break vector.
            let blocks = vec![RenderBlock::leaf(
                "text",
                json!({ "title": "#1 *bold* _it_ [x] $y$ \\ @at", "text": "Body /= line." }),
            )];
            assert_is_pdf(&compile_blocks(&blocks));
        }

        #[test]
        fn compilation_is_deterministic() {
            // Same source → identical bytes. `today()` returns None, so there is
            // no embedded timestamp to perturb the output.
            let source = build_typst_document(
                &LayoutMeta::default(),
                &[RenderBlock::leaf(
                    "text",
                    json!({ "title": "Repeatable", "text": "body" }),
                )],
            );
            let a = compile(&source).unwrap();
            let b = compile(&source).unwrap();
            assert_eq!(a, b, "same source must compile to identical PDF bytes");
        }

        #[test]
        fn distinct_sources_produce_distinct_output() {
            // Sanity check that the bytes actually reflect the input (so the
            // determinism test above isn't trivially passing on a constant).
            let a = compile("First document").unwrap();
            let b = compile("A completely different document with more text").unwrap();
            assert_ne!(a, b, "different sources should not collide");
        }
    }
}

/// The feature-off stub: there's no embedded compiler in this build, so report
/// it plainly rather than failing in a confusing way. Mirrors the `pdf` layer.
#[cfg(not(feature = "typst"))]
mod disabled {
    use crate::error::{AppError, AppResult};

    /// Compile a Typst source string to PDF bytes — unavailable without the
    /// `typst` feature. Returns `FeatureDisabled` so the renderer can show a
    /// clear "this build can't produce PDFs" message.
    pub fn compile(_source: &str) -> AppResult<Vec<u8>> {
        Err(AppError::FeatureDisabled { feature: "typst" })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::error::AppError;

        #[test]
        fn compile_without_feature_reports_feature_disabled() {
            let err = compile("anything").expect_err("disabled build must not compile");
            assert!(matches!(
                err,
                AppError::FeatureDisabled { feature: "typst" }
            ));
            assert_eq!(err.code(), "feature_disabled");
        }
    }
}
