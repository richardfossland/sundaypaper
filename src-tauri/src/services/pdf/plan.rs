//! Pure PDF planning logic — no PDF library needed, so it is always compiled
//! and unit-tested regardless of the `pdf` feature. The feature-gated engine
//! (`edit`, `render`) consumes the page lists this module produces.
//!
//! Page numbers are **1-based** everywhere the user sees them (matching how
//! print dialogs and humans count pages); the engine converts to 0-based when
//! talking to a library if needed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// Metadata about a PDF, returned by `pdf_info`.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/PdfInfo.ts")]
pub struct PdfInfo {
    /// Total number of pages.
    pub page_count: u32,
    /// Per-page size in PDF points (1/72 inch). May be empty if sizes could not
    /// be read; never partially trusted by callers for layout without a check.
    pub pages: Vec<PdfPageInfo>,
}

/// Size of a single page, in PDF points.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/PdfPageInfo.ts")]
pub struct PdfPageInfo {
    pub width_pt: f64,
    pub height_pt: f64,
}

/// Parse a 1-based page selection like `"1-3,5,8-10"` into an ordered, unique
/// list of page numbers, validated against `page_count`.
///
/// - An empty string or `"all"` selects every page in order.
/// - Order is preserved as written; duplicates are dropped (first wins) so
///   `"1,1,2"` → `[1, 2]`.
/// - Ranges may be ascending only; `"5-3"`, `0`, or a page past the end error.
pub fn parse_page_selection(spec: &str, page_count: u32) -> AppResult<Vec<u32>> {
    if page_count == 0 {
        return Err(AppError::Validation("the document has no pages".into()));
    }
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") {
        return Ok((1..=page_count).collect());
    }

    let mut out: Vec<u32> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let push = |n: u32, out: &mut Vec<u32>, seen: &mut std::collections::HashSet<u32>| {
        if seen.insert(n) {
            out.push(n);
        }
    };

    for raw in spec.split(',') {
        let part = raw.trim();
        if part.is_empty() {
            return Err(AppError::Validation(format!(
                "empty entry in page selection '{spec}'"
            )));
        }
        if let Some((a, b)) = part.split_once('-') {
            let start = parse_page_number(a.trim(), page_count)?;
            let end = parse_page_number(b.trim(), page_count)?;
            if start > end {
                return Err(AppError::Validation(format!(
                    "descending range '{part}' (start {start} > end {end})"
                )));
            }
            for n in start..=end {
                push(n, &mut out, &mut seen);
            }
        } else {
            let n = parse_page_number(part, page_count)?;
            push(n, &mut out, &mut seen);
        }
    }
    Ok(out)
}

fn parse_page_number(token: &str, page_count: u32) -> AppResult<u32> {
    let n: u32 = token
        .parse()
        .map_err(|_| AppError::Validation(format!("'{token}' is not a page number")))?;
    if n == 0 || n > page_count {
        return Err(AppError::Validation(format!(
            "page {n} is out of range (document has {page_count} pages)"
        )));
    }
    Ok(n)
}

/// Plan a "split every N pages" operation: partition `page_count` pages into
/// consecutive chunks of at most `chunk_size`, returning each chunk as a list
/// of 1-based page numbers. The last chunk holds the remainder.
pub fn plan_split_every(page_count: u32, chunk_size: u32) -> AppResult<Vec<Vec<u32>>> {
    if page_count == 0 {
        return Err(AppError::Validation("the document has no pages".into()));
    }
    if chunk_size == 0 {
        return Err(AppError::Validation("chunk size must be at least 1".into()));
    }
    let chunks = (1..=page_count)
        .collect::<Vec<_>>()
        .chunks(chunk_size as usize)
        .map(<[u32]>::to_vec)
        .collect();
    Ok(chunks)
}

/// Normalise a rotation in degrees to one of 0 / 90 / 180 / 270. Rejects
/// non-multiples of 90; accepts negative and >360 values.
pub fn normalize_rotation(degrees: i64) -> AppResult<i64> {
    if degrees % 90 != 0 {
        return Err(AppError::Validation(format!(
            "rotation must be a multiple of 90° (got {degrees})"
        )));
    }
    Ok(degrees.rem_euclid(360))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_all_and_empty() {
        assert_eq!(parse_page_selection("", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_page_selection("all", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_page_selection("ALL", 2).unwrap(), vec![1, 2]);
    }

    #[test]
    fn selection_ranges_and_dedup_preserve_order() {
        assert_eq!(
            parse_page_selection("1-3,5,8-10", 10).unwrap(),
            vec![1, 2, 3, 5, 8, 9, 10]
        );
        // Order preserved as written; duplicates dropped.
        assert_eq!(parse_page_selection("3,1,1,2", 3).unwrap(), vec![3, 1, 2]);
    }

    #[test]
    fn selection_rejects_bad_input() {
        assert!(parse_page_selection("0", 3).is_err());
        assert!(parse_page_selection("4", 3).is_err());
        assert!(parse_page_selection("5-3", 10).is_err());
        assert!(parse_page_selection("1,,2", 3).is_err());
        assert!(parse_page_selection("x", 3).is_err());
        assert!(parse_page_selection("1", 0).is_err());
    }

    #[test]
    fn split_every_partitions_with_remainder() {
        assert_eq!(
            plan_split_every(5, 2).unwrap(),
            vec![vec![1, 2], vec![3, 4], vec![5]]
        );
        assert_eq!(plan_split_every(4, 4).unwrap(), vec![vec![1, 2, 3, 4]]);
        assert_eq!(
            plan_split_every(3, 1).unwrap(),
            vec![vec![1], vec![2], vec![3]]
        );
        assert!(plan_split_every(5, 0).is_err());
        assert!(plan_split_every(0, 2).is_err());
    }

    #[test]
    fn rotation_normalises_and_validates() {
        assert_eq!(normalize_rotation(0).unwrap(), 0);
        assert_eq!(normalize_rotation(90).unwrap(), 90);
        assert_eq!(normalize_rotation(360).unwrap(), 0);
        assert_eq!(normalize_rotation(450).unwrap(), 90);
        assert_eq!(normalize_rotation(-90).unwrap(), 270);
        assert!(normalize_rotation(45).is_err());
    }
}
