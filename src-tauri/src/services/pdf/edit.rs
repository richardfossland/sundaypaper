//! lopdf-backed PDF manipulation: info, extract, split, rotate, merge. Pure
//! Rust (no external binary), so every operation here is round-trip tested.
//! Compiled only under the `pdf` feature.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use lopdf::{Document, Object, ObjectId};

use crate::error::{AppError, AppResult};
use crate::services::pdf::plan::{PdfInfo, PdfPageInfo};

fn pdf_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Pdf(e.to_string())
}

fn load(path: &Path) -> AppResult<Document> {
    Document::load(path).map_err(pdf_err)
}

fn page_count(doc: &Document) -> u32 {
    doc.get_pages().len() as u32
}

/// Read page count and best-effort per-page sizes.
pub fn info(path: &Path) -> AppResult<PdfInfo> {
    let doc = load(path)?;
    let pages = doc.get_pages();
    let mut sizes = Vec::with_capacity(pages.len());
    for page_id in pages.values() {
        if let Some(size) = page_size(&doc, *page_id) {
            sizes.push(size);
        }
    }
    // Only expose sizes if we read them for every page — a partial list would
    // mislead a caller indexing by page number.
    let pages_out = if sizes.len() == pages.len() {
        sizes
    } else {
        Vec::new()
    };
    Ok(PdfInfo {
        page_count: pages.len() as u32,
        pages: pages_out,
    })
}

/// Write a new PDF containing only `selection` (1-based), **in the order
/// given**. `delete_pages` alone only removes pages — it never reorders the
/// survivors — so after deleting we rebuild the Pages `Kids` array in the
/// requested order. This honours the documented contract that
/// `extract(&[3, 1])` yields page 3 followed by page 1.
pub fn extract(path: &Path, selection: &[u32], out: &Path) -> AppResult<()> {
    let mut doc = load(path)?;
    let total = page_count(&doc);
    let keep: HashSet<u32> = selection.iter().copied().collect();
    let to_delete: Vec<u32> = (1..=total).filter(|n| !keep.contains(n)).collect();
    if to_delete.len() as u32 == total {
        return Err(AppError::Validation(
            "selection would remove every page".into(),
        ));
    }

    // Map each original 1-based page number to its ObjectId *before* deleting,
    // so we can re-emit the survivors in the requested order afterwards.
    let original_pages = doc.get_pages();
    let ordered_ids: Vec<ObjectId> = selection
        .iter()
        .filter_map(|n| original_pages.get(n).copied())
        // De-dup while preserving first-seen order (a selection may repeat a
        // page; parse_page_selection already drops dupes, but be defensive).
        .fold(Vec::new(), |mut acc, id| {
            if !acc.contains(&id) {
                acc.push(id);
            }
            acc
        });

    doc.delete_pages(&to_delete);

    // Reorder the Pages node's Kids to match the requested order. The page
    // ObjectIds survive `delete_pages` unchanged; only the tree links change.
    reorder_pages(&mut doc, &ordered_ids)?;

    doc.save(out).map_err(pdf_err)?;
    Ok(())
}

/// Rewrite the root Pages node's `Kids` array to the given page ObjectIds, in
/// order, leaving `Count` consistent. Used by `extract` to honour the requested
/// page order (lopdf's `delete_pages` preserves original order otherwise).
fn reorder_pages(doc: &mut Document, ordered_ids: &[ObjectId]) -> AppResult<()> {
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let pages_ref = doc
        .get_object(root)
        .and_then(Object::as_dict)
        .and_then(|d| d.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let kids: Vec<Object> = ordered_ids
        .iter()
        .map(|id| Object::Reference(*id))
        .collect();
    let pages_dict = doc
        .get_object_mut(pages_ref)
        .map_err(pdf_err)?
        .as_dict_mut()
        .map_err(pdf_err)?;
    pages_dict.set("Count", Object::Integer(kids.len() as i64));
    pages_dict.set("Kids", Object::Array(kids));
    Ok(())
}

/// Write one output PDF per chunk. Returns the output paths in order.
/// Files are named `{stem}_{NN}.pdf` inside `out_dir`.
pub fn split(
    path: &Path,
    chunks: &[Vec<u32>],
    out_dir: &Path,
    stem: &str,
) -> AppResult<Vec<String>> {
    let doc = load(path)?;
    let total = page_count(&doc);
    let mut outputs = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let keep: HashSet<u32> = chunk.iter().copied().collect();
        let to_delete: Vec<u32> = (1..=total).filter(|n| !keep.contains(n)).collect();
        let mut part = doc.clone();
        part.delete_pages(&to_delete);
        let out_path = out_dir.join(format!("{stem}_{:02}.pdf", i + 1));
        part.save(&out_path).map_err(pdf_err)?;
        outputs.push(out_path.to_string_lossy().into_owned());
    }
    Ok(outputs)
}

/// Set `/Rotate` on each selected page (1-based) to `degrees` (already
/// normalised to 0/90/180/270 by the caller) and write the result.
pub fn rotate(path: &Path, selection: &[u32], degrees: i64, out: &Path) -> AppResult<()> {
    let mut doc = load(path)?;
    let pages = doc.get_pages();
    for n in selection {
        let id = *pages
            .get(n)
            .ok_or_else(|| AppError::Pdf(format!("page {n} not found")))?;
        let dict = doc
            .get_object_mut(id)
            .map_err(pdf_err)?
            .as_dict_mut()
            .map_err(pdf_err)?;
        dict.set("Rotate", Object::Integer(degrees));
    }
    doc.save(out).map_err(pdf_err)?;
    Ok(())
}

/// Merge several PDFs into one, in the given order. Follows the canonical lopdf
/// merge approach: renumber each input's objects into a shared id space, then
/// build a fresh Catalog + Pages tree pointing at every collected page.
pub fn merge(inputs: &[String], out: &Path) -> AppResult<()> {
    if inputs.len() < 2 {
        return Err(AppError::Validation(
            "merge needs at least two input PDFs".into(),
        ));
    }

    let mut max_id = 1u32;
    let mut documents_pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for path in inputs {
        let mut doc = Document::load(path).map_err(pdf_err)?;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;
        documents_pages.extend(
            doc.get_pages()
                .into_values()
                .map(|object_id| (object_id, doc.get_object(object_id).unwrap().to_owned())),
        );
        documents_objects.extend(doc.objects);
    }

    // Locate the existing Catalog and (first) Pages node; copy every other
    // non-page object straight across.
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in &documents_objects {
        match object.type_name().unwrap_or_default() {
            b"Catalog" => {
                catalog_object = Some((*object_id, object.clone()));
            }
            b"Pages" => {
                if pages_object.is_none() {
                    pages_object = Some((*object_id, object.clone()));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    let (catalog_id, catalog_obj) =
        catalog_object.ok_or_else(|| AppError::Pdf("no Catalog found in inputs".into()))?;
    let (pages_id, pages_obj) =
        pages_object.ok_or_else(|| AppError::Pdf("no Pages node found in inputs".into()))?;

    // Reparent every collected page onto the shared Pages node and copy it in.
    for (page_id, page_obj) in &documents_pages {
        if let Ok(dict) = page_obj.as_dict() {
            let mut dict = dict.clone();
            dict.set("Parent", pages_id);
            document.objects.insert(*page_id, Object::Dictionary(dict));
        }
    }

    // Rebuild the Pages node with the full Kids list and Count.
    let mut pages_dict = pages_obj.as_dict().map_err(pdf_err)?.clone();
    let kids: Vec<Object> = documents_pages
        .keys()
        .map(|id| Object::Reference(*id))
        .collect();
    pages_dict.set("Count", Object::Integer(kids.len() as i64));
    pages_dict.set("Kids", Object::Array(kids));
    document
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));

    // Catalog points at the shared Pages node.
    let mut catalog_dict = catalog_obj.as_dict().map_err(pdf_err)?.clone();
    catalog_dict.set("Pages", pages_id);
    document
        .objects
        .insert(catalog_id, Object::Dictionary(catalog_dict));

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.keys().map(|(n, _)| *n).max().unwrap_or(0);
    document.renumber_objects();
    document.compress();
    document.save(out).map_err(pdf_err)?;
    Ok(())
}

/// Best-effort page size in points, following MediaBox inheritance up the
/// page-tree Parent chain. Returns `None` if no MediaBox is found.
fn page_size(doc: &Document, page_id: ObjectId) -> Option<PdfPageInfo> {
    let mb = media_box(doc, page_id)?;
    Some(PdfPageInfo {
        width_pt: (mb[2] - mb[0]).abs(),
        height_pt: (mb[3] - mb[1]).abs(),
    })
}

fn media_box(doc: &Document, start: ObjectId) -> Option<[f64; 4]> {
    let mut current = Some(start);
    for _ in 0..16 {
        let id = current?;
        let dict = doc.get_object(id).ok()?.as_dict().ok()?;
        if let Ok(mb) = dict.get(b"MediaBox") {
            let arr = mb.as_array().ok()?;
            if arr.len() == 4 {
                let mut v = [0.0f64; 4];
                for (i, item) in arr.iter().enumerate() {
                    v[i] = number(item)?;
                }
                return Some(v);
            }
        }
        current = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    None
}

fn number(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Content;
    use lopdf::{dictionary, Stream};

    /// Build a minimal valid `n`-page PDF at `path` (A4 pages, empty content).
    fn build_pdf(path: &Path, n: u32) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..n {
            let content = Content { operations: vec![] };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        finish_pdf(&mut doc, path, pages_id, kids);
    }

    /// Build an `n`-page PDF where page `i` (1-based) has a distinctive MediaBox
    /// width of `100 + i` points, so a test can identify which original page
    /// ended up at each output position (content/order verification).
    fn build_marked_pdf(path: &Path, n: u32) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for i in 1..=n {
            let content = Content { operations: vec![] };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let width = (100 + i) as i64;
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), width.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        finish_pdf(&mut doc, path, pages_id, kids);
    }

    /// Build a 2-page PDF whose page ObjectIds run *opposite* to the visual
    /// (Kids) order: the first page (MediaBox width 201) gets a HIGHER ObjectId
    /// than the second page (width 202). Linearized/optimised PDFs routinely do
    /// this. Used to prove merge preserves page order, not ObjectId order.
    fn build_reverse_id_pdf(path: &Path) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        // Allocate the SECOND visual page first -> it gets the lower ObjectId.
        let content_b = Content { operations: vec![] };
        let content_b_id = doc.add_object(Stream::new(dictionary! {}, content_b.encode().unwrap()));
        let page_b = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_b_id,
            "MediaBox" => vec![0.into(), 0.into(), 202.into(), 842.into()],
        });

        // Allocate the FIRST visual page second -> it gets the higher ObjectId.
        let content_a = Content { operations: vec![] };
        let content_a_id = doc.add_object(Stream::new(dictionary! {}, content_a.encode().unwrap()));
        let page_a = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_a_id,
            "MediaBox" => vec![0.into(), 0.into(), 201.into(), 842.into()],
        });

        // Sanity: page_a (visual first) really does have the higher ObjectId.
        assert!(
            page_a.0 > page_b.0,
            "fixture must reverse id vs visual order"
        );

        // Kids in VISUAL order: page A (width 201) first, page B (width 202) next.
        let kids = vec![page_a.into(), page_b.into()];
        finish_pdf(&mut doc, path, pages_id, kids);
    }

    fn finish_pdf(doc: &mut Document, path: &Path, pages_id: ObjectId, kids: Vec<Object>) {
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
        doc.save(path).expect("save fixture");
    }

    fn count_pages(path: &Path) -> u32 {
        Document::load(path).expect("load").get_pages().len() as u32
    }

    /// Read the MediaBox widths of every page in document (Kids) order — the
    /// marker `build_marked_pdf` writes, used to assert page *order*.
    fn page_widths(path: &Path) -> Vec<f64> {
        let doc = Document::load(path).expect("load");
        doc.get_pages()
            .values()
            .map(|id| {
                let dict = doc.get_object(*id).unwrap().as_dict().unwrap();
                let mb = dict.get(b"MediaBox").unwrap().as_array().unwrap();
                number(&mb[2]).unwrap()
            })
            .collect()
    }

    #[test]
    fn info_reads_count_and_a4_size() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        build_pdf(&src, 3);
        let info = info(&src).unwrap();
        assert_eq!(info.page_count, 3);
        assert_eq!(info.pages.len(), 3);
        assert_eq!(info.pages[0].width_pt, 595.0);
        assert_eq!(info.pages[0].height_pt, 842.0);
    }

    #[test]
    fn extract_keeps_only_selected() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        let out = dir.path().join("out.pdf");
        build_pdf(&src, 5);
        extract(&src, &[2, 4], &out).unwrap();
        assert_eq!(count_pages(&out), 2);
    }

    #[test]
    fn extract_honours_requested_page_order() {
        // pdf_ops::extract_pages and parse_page_selection both document that the
        // selection order is preserved: extracting [3, 1] must yield page 3 then
        // page 1. Pages carry distinctive MediaBox widths (101/102/103) so we can
        // tell which original page landed at each output slot.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        let out = dir.path().join("out.pdf");
        build_marked_pdf(&src, 3);
        extract(&src, &[3, 1], &out).unwrap();
        assert_eq!(
            page_widths(&out),
            vec![103.0, 101.0],
            "extracted pages must follow the requested order [3, 1]"
        );
    }

    #[test]
    fn extract_rejects_removing_everything() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        let out = dir.path().join("out.pdf");
        build_pdf(&src, 2);
        assert!(matches!(
            extract(&src, &[], &out).unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[test]
    fn split_writes_one_file_per_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        build_pdf(&src, 5);
        let chunks = vec![vec![1, 2], vec![3, 4], vec![5]];
        let outs = split(&src, &chunks, dir.path(), "part").unwrap();
        assert_eq!(outs.len(), 3);
        assert_eq!(count_pages(Path::new(&outs[0])), 2);
        assert_eq!(count_pages(Path::new(&outs[2])), 1);
    }

    #[test]
    fn rotate_sets_rotate_on_selected_pages() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.pdf");
        let out = dir.path().join("out.pdf");
        build_pdf(&src, 2);
        rotate(&src, &[1], 90, &out).unwrap();
        let doc = Document::load(&out).unwrap();
        let pages = doc.get_pages();
        let first = *pages.get(&1).unwrap();
        let dict = doc.get_object(first).unwrap().as_dict().unwrap();
        assert_eq!(dict.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
    }

    #[test]
    fn merge_preserves_visual_page_order_over_object_id_order() {
        // A reverse-id input: visual page 1 has a HIGHER ObjectId than page 2.
        // merge must emit pages in visual (Kids) order, not ObjectId order.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        let out = dir.path().join("merged.pdf");
        build_reverse_id_pdf(&a);
        build_pdf(&b, 1);
        merge(
            &[a.to_string_lossy().into(), b.to_string_lossy().into()],
            &out,
        )
        .unwrap();
        let widths = page_widths(&out);
        assert_eq!(widths.len(), 3, "two pages from A + one from B");
        // The first two pages must follow A's Kids order (201 then 202), not the
        // ObjectId order (which would scramble them to 202 then 201).
        assert_eq!(
            &widths[..2],
            &[201.0, 202.0],
            "merged pages must follow visual order, not ObjectId order"
        );
    }

    #[test]
    fn merge_sums_page_counts() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        let out = dir.path().join("merged.pdf");
        build_pdf(&a, 2);
        build_pdf(&b, 3);
        merge(
            &[a.to_string_lossy().into(), b.to_string_lossy().into()],
            &out,
        )
        .unwrap();
        assert_eq!(count_pages(&out), 5);
    }
}
