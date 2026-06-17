//! Sangbok-klipper pipeline (Phase 3.1 OCR prep).
//!
//! Manages the lifecycle of a `SangbokJob`: an OCR pipeline run on a scanned
//! hymnal/song-book PDF. The actual OCR step (Tesseract) is hardware-bound and
//! stays behind the `ocr` cargo feature; this module provides:
//!
//! - The state-machine: `Queued → Processing → Done | Failed`
//! - Persistence: `sangbok_job` + `song_extract` tables (migration 0003)
//! - A stub `process` that sets the job to `Done` with an empty extract list
//!   and a `not_implemented` detail — real OCR replaces this body later.
//! - Tauri commands: `sangbok_import`, `sangbok_list_jobs`, `sangbok_get_job`,
//!   `sangbok_cancel`
//!
//! Tests cover the state-machine, validation, and the stub response, with no
//! dependency on native OCR libraries or file-system paths.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

// ── Status enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/SangbokJobStatus.ts")]
pub enum SangbokJobStatus {
    Queued,
    Processing,
    Done,
    Failed,
}

impl SangbokJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SangbokJobStatus::Queued => "Queued",
            SangbokJobStatus::Processing => "Processing",
            SangbokJobStatus::Done => "Done",
            SangbokJobStatus::Failed => "Failed",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "Queued" => Ok(SangbokJobStatus::Queued),
            "Processing" => Ok(SangbokJobStatus::Processing),
            "Done" => Ok(SangbokJobStatus::Done),
            "Failed" => Ok(SangbokJobStatus::Failed),
            other => Err(AppError::Validation(format!(
                "unknown SangbokJob status '{other}'"
            ))),
        }
    }

    /// Returns `true` if the job is in a terminal state and cannot be
    /// cancelled or advanced.
    pub fn is_terminal(&self) -> bool {
        matches!(self, SangbokJobStatus::Done | SangbokJobStatus::Failed)
    }
}

// ── SongExtract ───────────────────────────────────────────────────────────────

/// A song found (or hypothesised) within a sangbok PDF.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/SongExtract.ts")]
pub struct SongExtract {
    pub id: String,
    pub job_id: String,
    /// First page (0-indexed) in the source PDF.
    pub page_start: usize,
    /// Last page (inclusive) in the source PDF.
    pub page_end: usize,
    /// Best-guess song title from OCR text or heuristic.
    pub title_hint: String,
    /// OCR/heuristic confidence ∈ [0.0, 1.0].
    pub confidence: f32,
    pub position: i64,
}

// ── SangbokJob ────────────────────────────────────────────────────────────────

/// An OCR pipeline job for a scanned hymnal PDF.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/SangbokJob.ts")]
pub struct SangbokJob {
    pub id: String,
    pub input_pdf_path: String,
    pub status: String,
    /// Page count as reported by the PDF reader (0 until determined).
    pub page_count: usize,
    pub songs_found: Vec<SongExtract>,
    /// Detail / error message — `None` while still running.
    pub error_detail: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── SQLite row shapes ─────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SangbokJobRow {
    id: String,
    input_pdf_path: String,
    status: String,
    page_count: i64,
    error_detail: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(sqlx::FromRow)]
struct SongExtractRow {
    id: String,
    job_id: String,
    page_start: i64,
    page_end: i64,
    title_hint: String,
    confidence: f64,
    position: i64,
}

// ── Repo ─────────────────────────────────────────────────────────────────────

pub struct SangbokRepo {
    pub db: Db,
}

impl SangbokRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    // ── Import (queue a new job) ──────────────────────────────────────────────

    /// Queue a new job for the given PDF path. Returns the persisted job in
    /// `Queued` status. Validation: path must be non-empty.
    pub async fn import(&self, pdf_path: &str) -> AppResult<SangbokJob> {
        let pdf_path = pdf_path.trim();
        if pdf_path.is_empty() {
            return Err(AppError::Validation(
                "sangbok: input_pdf_path is required".into(),
            ));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO sangbok_job (id, input_pdf_path, status, page_count, created_at, updated_at) \
             VALUES (?, ?, 'Queued', 0, ?, ?)",
        )
        .bind(&id)
        .bind(pdf_path)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;

        self.get(&id).await
    }

    // ── List ──────────────────────────────────────────────────────────────────

    /// All jobs, newest first.
    pub async fn list(&self) -> AppResult<Vec<SangbokJob>> {
        let rows = sqlx::query_as::<_, SangbokJobRow>(
            "SELECT id, input_pdf_path, status, page_count, error_detail, created_at, updated_at \
             FROM sangbok_job ORDER BY created_at DESC, id DESC",
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let extracts = self.load_extracts(&row.id).await?;
            out.push(row_to_job(row, extracts));
        }
        Ok(out)
    }

    // ── Get ───────────────────────────────────────────────────────────────────

    pub async fn get(&self, id: &str) -> AppResult<SangbokJob> {
        let row = sqlx::query_as::<_, SangbokJobRow>(
            "SELECT id, input_pdf_path, status, page_count, error_detail, created_at, updated_at \
             FROM sangbok_job WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db.pool)
        .await?
        .ok_or_else(|| AppError::NotFound {
            entity: "sangbok_job",
            id: id.to_string(),
        })?;

        let extracts = self.load_extracts(id).await?;
        Ok(row_to_job(row, extracts))
    }

    // ── Cancel ────────────────────────────────────────────────────────────────

    /// Cancel a job that is still in `Queued` or `Processing` state. Moves it
    /// to `Failed` with a "cancelled" detail. Terminal jobs cannot be cancelled.
    pub async fn cancel(&self, id: &str) -> AppResult<SangbokJob> {
        let job = self.get(id).await?;
        if SangbokJobStatus::parse(&job.status)?.is_terminal() {
            return Err(AppError::Validation(format!(
                "sangbok_job {id} is already in terminal state '{}'",
                job.status
            )));
        }
        let now = now_ms();
        sqlx::query(
            "UPDATE sangbok_job SET status = 'Failed', error_detail = 'cancelled', updated_at = ? \
             WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(&self.db.pool)
        .await?;
        self.get(id).await
    }

    // ── Process (OCR stub) ────────────────────────────────────────────────────

    /// Run the OCR pipeline for a job. This is a **stub**: it advances the job
    /// to `Processing`, then immediately to `Done` with an empty `songs_found`
    /// list and a `not_implemented` detail. Real Tesseract OCR replaces this
    /// body when the `ocr` cargo feature lands (Phase 3.1).
    pub async fn process(&self, id: &str) -> AppResult<SangbokJob> {
        let job = self.get(id).await?;
        if SangbokJobStatus::parse(&job.status)?.is_terminal() {
            return Err(AppError::Validation(format!(
                "sangbok_job {id} cannot be processed from terminal state '{}'",
                job.status
            )));
        }

        let now = now_ms();
        // Advance to Processing.
        sqlx::query("UPDATE sangbok_job SET status = 'Processing', updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(&self.db.pool)
            .await?;

        // OCR STUB — no actual OCR performed.
        // The state-machine transitions to Done; songs_found stays empty.
        // The `not_implemented` detail signals to callers that the OCR step
        // requires the `ocr` cargo feature and Tesseract to be installed.
        sqlx::query(
            "UPDATE sangbok_job \
             SET status = 'Done', error_detail = 'not_implemented: OCR requires the ocr feature', \
                 updated_at = ? \
             WHERE id = ?",
        )
        .bind(now + 1)
        .bind(id)
        .execute(&self.db.pool)
        .await?;

        self.get(id).await
    }

    // ── Process from page text (OCR-free core) ────────────────────────────────

    /// Segment a job from already-extracted per-page text and persist the
    /// resulting song boundaries as `SongExtract` rows, then mark the job
    /// `Done`. This is the OCR-free core the real worker calls once Tesseract
    /// (behind the `ocr` feature) has turned the PDF's pages into `pages`; it
    /// reads no files itself, so it is fully offline-testable. The boundary
    /// logic lives in the pure [`segment_songs`].
    pub async fn process_pages(&self, id: &str, pages: &[String]) -> AppResult<SangbokJob> {
        let job = self.get(id).await?;
        if SangbokJobStatus::parse(&job.status)?.is_terminal() {
            return Err(AppError::Validation(format!(
                "sangbok_job {id} cannot be processed from terminal state '{}'",
                job.status
            )));
        }

        // Advance to Processing and record the page count we were handed.
        self.set_status(id, &SangbokJobStatus::Processing, None, Some(pages.len()))
            .await?;

        let boundaries = segment_songs(pages);
        for (position, b) in boundaries.iter().enumerate() {
            // Prefer the parsed number as the title hint when no title text was
            // recovered, so a bare numbered page is still identifiable.
            let title_hint = if b.title_hint.is_empty() {
                b.number.map(|n| n.to_string()).unwrap_or_default()
            } else {
                b.title_hint.clone()
            };
            self.add_extract(
                id,
                b.page_start,
                b.page_end,
                &title_hint,
                b.confidence,
                position as i64,
            )
            .await?;
        }

        let detail = format!("segmented {} song(s)", boundaries.len());
        self.set_status(id, &SangbokJobStatus::Done, Some(&detail), None)
            .await
    }

    // ── Advance status (internal, used in tests) ──────────────────────────────

    /// Directly set a job's status and optional detail. Used in tests and by
    /// the real OCR worker when it replaces the stub.
    pub async fn set_status(
        &self,
        id: &str,
        status: &SangbokJobStatus,
        detail: Option<&str>,
        page_count: Option<usize>,
    ) -> AppResult<SangbokJob> {
        let pc_bind: Option<i64> = page_count.map(|c| c as i64);
        let now = now_ms();
        let affected = if let Some(pc) = pc_bind {
            sqlx::query(
                "UPDATE sangbok_job \
                 SET status = ?, error_detail = ?, page_count = ?, updated_at = ? \
                 WHERE id = ?",
            )
            .bind(status.as_str())
            .bind(detail)
            .bind(pc)
            .bind(now)
            .bind(id)
            .execute(&self.db.pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "UPDATE sangbok_job SET status = ?, error_detail = ?, updated_at = ? WHERE id = ?",
            )
            .bind(status.as_str())
            .bind(detail)
            .bind(now)
            .bind(id)
            .execute(&self.db.pool)
            .await?
            .rows_affected()
        };

        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "sangbok_job",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Insert a `SongExtract` for a job. Used by the real OCR worker when it
    /// replaces the stub.
    pub async fn add_extract(
        &self,
        job_id: &str,
        page_start: usize,
        page_end: usize,
        title_hint: &str,
        confidence: f32,
        position: i64,
    ) -> AppResult<SongExtract> {
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO song_extract (id, job_id, page_start, page_end, title_hint, confidence, position) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(job_id)
        .bind(page_start as i64)
        .bind(page_end as i64)
        .bind(title_hint)
        .bind(confidence as f64)
        .bind(position)
        .execute(&self.db.pool)
        .await?;

        // Update the job's updated_at so the UI refreshes.
        sqlx::query("UPDATE sangbok_job SET updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(job_id)
            .execute(&self.db.pool)
            .await?;

        Ok(SongExtract {
            id,
            job_id: job_id.to_string(),
            page_start,
            page_end,
            title_hint: title_hint.to_string(),
            confidence,
            position,
        })
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    async fn load_extracts(&self, job_id: &str) -> AppResult<Vec<SongExtract>> {
        let rows = sqlx::query_as::<_, SongExtractRow>(
            "SELECT id, job_id, page_start, page_end, title_hint, confidence, position \
             FROM song_extract WHERE job_id = ? ORDER BY position ASC, id ASC",
        )
        .bind(job_id)
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_extract).collect())
    }
}

// ── Row mappers ───────────────────────────────────────────────────────────────

fn row_to_job(row: SangbokJobRow, extracts: Vec<SongExtract>) -> SangbokJob {
    SangbokJob {
        id: row.id,
        input_pdf_path: row.input_pdf_path,
        status: row.status,
        page_count: row.page_count as usize,
        songs_found: extracts,
        error_detail: row.error_detail,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn row_to_extract(row: SongExtractRow) -> SongExtract {
    SongExtract {
        id: row.id,
        job_id: row.job_id,
        page_start: row.page_start as usize,
        page_end: row.page_end as usize,
        title_hint: row.title_hint,
        confidence: row.confidence as f32,
        position: row.position,
    }
}

// ── Song-boundary segmentation (pure, OCR-free) ─────────────────────────────────

/// A song boundary the segmenter found within a run of OCR'd page texts. This is
/// the pure result of [`segment_songs`] — it carries the range and a best-guess
/// title/number but no DB identity, so it can be unit-tested without Tesseract,
/// a PDF, or a database. The persistence layer turns each one into a
/// [`SongExtract`] row.
#[derive(Debug, Clone, PartialEq)]
pub struct SongBoundary {
    /// First page (0-indexed) belonging to this song.
    pub page_start: usize,
    /// Last page (inclusive) belonging to this song.
    pub page_end: usize,
    /// Best-guess title from the page's first lines (empty if none detected).
    pub title_hint: String,
    /// Hymnal / songbook number parsed off the title line, if present
    /// (e.g. `97` from `N13 097`).
    pub number: Option<u32>,
    /// Heuristic confidence ∈ [0.0, 1.0] that this is a real song boundary.
    pub confidence: f32,
}

/// Split a sequence of per-page OCR texts into song boundaries — the pure,
/// hardware-free heart of the Sangbok-klipper. The OCR step (Tesseract) is
/// gated behind the `ocr` feature and produces the `pages` slice; this function
/// turns that text into ranges and needs no native libraries.
///
/// Heuristic: a page **starts a new song** when its first non-blank line looks
/// like a song header. Two signals, each justified by how hymnals are laid out:
///
/// 1. A leading hymnal number — a short token of digits, optionally prefixed by
///    a book code like `N13` (e.g. `N13 097`, `97.`, `# 123`). High confidence.
/// 2. A short Title-Case line (a handful of words, most capitalised) with the
///    rest of the page below it. Medium confidence.
///
/// Pages that match neither are treated as continuation pages of the song in
/// progress (a long hymn spilling onto the next page). If the very first page
/// has no detectable header, a single low-confidence fallback boundary spanning
/// the whole document is returned so the splitter still yields something usable.
pub fn segment_songs(pages: &[String]) -> Vec<SongBoundary> {
    let mut boundaries: Vec<SongBoundary> = Vec::new();

    for (idx, page) in pages.iter().enumerate() {
        match detect_header(page) {
            Some((title_hint, number, confidence)) => {
                // Close the previous boundary at the page before this one.
                if let Some(prev) = boundaries.last_mut() {
                    prev.page_end = idx.saturating_sub(1);
                }
                boundaries.push(SongBoundary {
                    page_start: idx,
                    page_end: idx,
                    title_hint,
                    number,
                    confidence,
                });
            }
            None => {
                // Continuation page: extend the current song, or — only when the
                // document opened with no header at all — seed a low-confidence
                // fallback that the following pages keep extending.
                match boundaries.last_mut() {
                    Some(cur) => cur.page_end = idx,
                    None => boundaries.push(SongBoundary {
                        page_start: idx,
                        page_end: idx,
                        title_hint: String::new(),
                        number: None,
                        confidence: 0.2,
                    }),
                }
            }
        }
    }

    boundaries
}

/// Inspect a page's first non-blank line for a song header. Returns
/// `(title_hint, number, confidence)` when it looks like the start of a song.
fn detect_header(page: &str) -> Option<(String, Option<u32>, f32)> {
    let line = page.lines().map(str::trim).find(|l| !l.is_empty())?;

    // Signal 1: a leading hymnal number, optionally with a book-code prefix.
    if let Some((number, rest)) = parse_leading_number(line) {
        // The remainder of the line (after the number) is the title, if any.
        let title_hint = if rest.is_empty() {
            line.to_string()
        } else {
            rest.to_string()
        };
        return Some((title_hint, Some(number), 0.9));
    }

    // Signal 2: a short, mostly Title-Case line reads like a song title.
    if looks_like_title(line) {
        return Some((line.to_string(), None, 0.6));
    }

    None
}

/// Parse a leading hymnal number off a header line, returning the number and the
/// trailing title text. Accepts an optional book-code prefix (a letter followed
/// by digits, e.g. `N13`) before the song number, and an optional trailing
/// punctuation (`.`/`)`). Examples: `N13 097 Holy Night` → `(97, "Holy Night")`,
/// `97. Amazing Grace` → `(97, "Amazing Grace")`.
fn parse_leading_number(line: &str) -> Option<(u32, String)> {
    let mut tokens = line.split_whitespace();
    let first = tokens.next()?;

    // Optional `#` marker token on its own (e.g. "# 123").
    let (first, rest_tokens): (&str, Vec<&str>) = if first == "#" {
        (tokens.next()?, tokens.collect())
    } else {
        (first, tokens.collect())
    };

    // A leading book code like `N13` is a single uppercase letter + digits; if
    // present, the real song number is the next token.
    let (num_token, title_tokens): (&str, &[&str]) =
        if is_book_code(first) && !rest_tokens.is_empty() {
            (rest_tokens[0], &rest_tokens[1..])
        } else {
            (first, &rest_tokens[..])
        };

    let digits: String = num_token
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    // Guard against absurdly long "numbers" (likely OCR noise, not a hymn no.).
    if digits.len() > 4 {
        return None;
    }
    let number: u32 = digits.parse().ok()?;
    Some((number, title_tokens.join(" ").trim().to_string()))
}

/// A book code is a short run of letters followed by digits (e.g. `N13`, `S97`).
fn is_book_code(tok: &str) -> bool {
    let mut chars = tok.chars();
    let mut saw_letter = false;
    let mut saw_digit = false;
    for c in chars.by_ref() {
        if c.is_ascii_alphabetic() && !saw_digit {
            saw_letter = true;
        } else if c.is_ascii_digit() {
            saw_digit = true;
        } else {
            return false;
        }
    }
    saw_letter && saw_digit
}

/// Heuristic: a line reads like a song title when it is short (≤ 8 words) and
/// most of its alphabetic words are capitalised — the way hymnal titles are set.
fn looks_like_title(line: &str) -> bool {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.is_empty() || words.len() > 8 {
        return false;
    }
    let alpha_words: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| w.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
        .collect();
    if alpha_words.is_empty() {
        return false;
    }
    let capitalised = alpha_words
        .iter()
        .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
        .count();
    // At least two-thirds of the words start with a capital.
    capitalised * 3 >= alpha_words.len() * 2
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> SangbokRepo {
        SangbokRepo::new(Db::connect_memory().await.expect("connect"))
    }

    // ── Status enum ───────────────────────────────────────────────────────────

    #[test]
    fn status_round_trip() {
        for s in [
            SangbokJobStatus::Queued,
            SangbokJobStatus::Processing,
            SangbokJobStatus::Done,
            SangbokJobStatus::Failed,
        ] {
            let back = SangbokJobStatus::parse(s.as_str()).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn status_unknown_is_validation_error() {
        assert!(matches!(
            SangbokJobStatus::parse("Running").unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[test]
    fn terminal_states() {
        assert!(SangbokJobStatus::Done.is_terminal());
        assert!(SangbokJobStatus::Failed.is_terminal());
        assert!(!SangbokJobStatus::Queued.is_terminal());
        assert!(!SangbokJobStatus::Processing.is_terminal());
    }

    // ── Import / create ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn import_creates_queued_job() {
        let repo = repo().await;
        let job = repo.import("/path/to/sangbok.pdf").await.unwrap();
        assert_eq!(job.status, "Queued");
        assert_eq!(job.input_pdf_path, "/path/to/sangbok.pdf");
        assert_eq!(job.page_count, 0);
        assert!(job.songs_found.is_empty());
        assert!(job.error_detail.is_none());
    }

    #[tokio::test]
    async fn import_validates_path() {
        let repo = repo().await;
        assert!(matches!(
            repo.import("   ").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    // ── List / get ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_returns_newest_first() {
        let repo = repo().await;
        let a = repo.import("/a.pdf").await.unwrap();
        let b = repo.import("/b.pdf").await.unwrap();
        // pin distinct timestamps
        for (id, ts) in [(&a.id, 1_000i64), (&b.id, 2_000i64)] {
            sqlx::query("UPDATE sangbok_job SET created_at = ? WHERE id = ?")
                .bind(ts)
                .bind(id)
                .execute(&repo.db.pool)
                .await
                .unwrap();
        }
        let list = repo.list().await.unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }

    #[tokio::test]
    async fn get_not_found() {
        let repo = repo().await;
        assert!(matches!(
            repo.get("ghost-id").await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    // ── Process stub ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn process_stub_advances_to_done() {
        let repo = repo().await;
        let job = repo.import("/book.pdf").await.unwrap();
        assert_eq!(job.status, "Queued");
        let processed = repo.process(&job.id).await.unwrap();
        assert_eq!(processed.status, "Done");
        assert!(processed.songs_found.is_empty(), "stub: no extracts");
        assert!(
            processed
                .error_detail
                .as_deref()
                .unwrap_or("")
                .contains("not_implemented"),
            "stub signals not_implemented"
        );
    }

    #[tokio::test]
    async fn process_on_terminal_job_is_error() {
        let repo = repo().await;
        let job = repo.import("/x.pdf").await.unwrap();
        // manually advance to Done
        repo.set_status(&job.id, &SangbokJobStatus::Done, None, None)
            .await
            .unwrap();
        assert!(matches!(
            repo.process(&job.id).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    // ── Cancel ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cancel_queued_job_moves_to_failed() {
        let repo = repo().await;
        let job = repo.import("/c.pdf").await.unwrap();
        let cancelled = repo.cancel(&job.id).await.unwrap();
        assert_eq!(cancelled.status, "Failed");
        assert_eq!(cancelled.error_detail.as_deref(), Some("cancelled"));
    }

    #[tokio::test]
    async fn cancel_processing_job_moves_to_failed() {
        let repo = repo().await;
        let job = repo.import("/d.pdf").await.unwrap();
        repo.set_status(&job.id, &SangbokJobStatus::Processing, None, None)
            .await
            .unwrap();
        let cancelled = repo.cancel(&job.id).await.unwrap();
        assert_eq!(cancelled.status, "Failed");
    }

    #[tokio::test]
    async fn cancel_terminal_job_is_error() {
        let repo = repo().await;
        let job = repo.import("/e.pdf").await.unwrap();
        repo.set_status(&job.id, &SangbokJobStatus::Done, None, None)
            .await
            .unwrap();
        assert!(matches!(
            repo.cancel(&job.id).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    // ── SongExtract ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn extracts_are_loaded_with_job() {
        let repo = repo().await;
        let job = repo.import("/hymnal.pdf").await.unwrap();
        repo.add_extract(&job.id, 0, 2, "Amazing Grace", 0.92, 0)
            .await
            .unwrap();
        repo.add_extract(&job.id, 3, 4, "Holy, Holy, Holy", 0.85, 1)
            .await
            .unwrap();
        let fetched = repo.get(&job.id).await.unwrap();
        assert_eq!(fetched.songs_found.len(), 2);
        assert_eq!(fetched.songs_found[0].title_hint, "Amazing Grace");
        assert!((fetched.songs_found[0].confidence - 0.92).abs() < 0.01);
        assert_eq!(fetched.songs_found[1].title_hint, "Holy, Holy, Holy");
    }

    // ── Song-boundary segmentation (pure) ──────────────────────────────────────

    fn pages(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn segments_three_numbered_songs_into_three_ranges() {
        let p = pages(&[
            "N13 097 Holy, Holy, Holy\nHoly, holy, holy! Lord God Almighty",
            "...continuation of holy holy holy...",
            "N13 098 Amazing Grace\nAmazing grace, how sweet the sound",
            "N13 099 Be Thou My Vision\nBe thou my vision",
        ]);
        let songs = segment_songs(&p);
        assert_eq!(songs.len(), 3);
        // First song spans its header page + the continuation page.
        assert_eq!((songs[0].page_start, songs[0].page_end), (0, 1));
        assert_eq!(songs[0].number, Some(97));
        assert_eq!(songs[0].title_hint, "Holy, Holy, Holy");
        assert_eq!((songs[1].page_start, songs[1].page_end), (2, 2));
        assert_eq!(songs[1].number, Some(98));
        assert_eq!((songs[2].page_start, songs[2].page_end), (3, 3));
        assert_eq!(songs[2].number, Some(99));
        assert!(
            songs[0].confidence > 0.8,
            "numbered header → high confidence"
        );
    }

    #[test]
    fn parses_number_off_various_header_formats() {
        // Trailing punctuation, hash marker, plain number, book code.
        assert_eq!(
            parse_leading_number("97. Amazing Grace"),
            Some((97, "Amazing Grace".to_string()))
        );
        assert_eq!(
            parse_leading_number("# 123 To God Be the Glory"),
            Some((123, "To God Be the Glory".to_string()))
        );
        assert_eq!(
            parse_leading_number("N13 097 Holy Night"),
            Some((97, "Holy Night".to_string()))
        );
        // No leading number → None.
        assert_eq!(parse_leading_number("Just A Title"), None);
        // Absurdly long digit run is rejected as OCR noise.
        assert_eq!(parse_leading_number("123456 noise"), None);
    }

    #[test]
    fn title_case_line_is_a_medium_confidence_boundary() {
        let p = pages(&[
            "Amazing Grace\nAmazing grace, how sweet the sound\nthat saved a wretch like me",
            "Be Thou My Vision\nBe thou my vision, O Lord of my heart",
        ]);
        let songs = segment_songs(&p);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title_hint, "Amazing Grace");
        assert_eq!(songs[0].number, None);
        assert!(
            (songs[0].confidence - 0.6).abs() < 0.001,
            "title-only → medium"
        );
    }

    #[test]
    fn document_with_no_detectable_header_yields_low_confidence_fallback() {
        // Lowercase body text with no title-looking lines anywhere.
        let p = pages(&[
            "amazing grace, how sweet the sound that saved a wretch like me",
            "i once was lost but now am found, was blind but now i see",
        ]);
        let songs = segment_songs(&p);
        assert_eq!(songs.len(), 1, "single fallback boundary");
        assert_eq!((songs[0].page_start, songs[0].page_end), (0, 1));
        assert!(songs[0].title_hint.is_empty());
        assert!(songs[0].confidence < 0.3, "fallback is low confidence");
    }

    #[test]
    fn empty_input_yields_no_boundaries() {
        assert!(segment_songs(&[]).is_empty());
    }

    #[test]
    fn blank_first_line_is_skipped_when_detecting_header() {
        let p = pages(&["\n\n  97 Amazing Grace\nbody"]);
        let songs = segment_songs(&p);
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].number, Some(97));
        assert_eq!(songs[0].title_hint, "Amazing Grace");
    }

    #[tokio::test]
    async fn process_pages_persists_segmented_extracts() {
        let repo = repo().await;
        let job = repo.import("/hymnal.pdf").await.unwrap();
        let p = pages(&[
            "97 Amazing Grace\nAmazing grace",
            "98 Be Thou My Vision\nBe thou my vision",
        ]);
        let done = repo.process_pages(&job.id, &p).await.unwrap();
        assert_eq!(done.status, "Done");
        assert_eq!(done.page_count, 2);
        assert_eq!(done.songs_found.len(), 2);
        assert_eq!(done.songs_found[0].title_hint, "Amazing Grace");
        assert_eq!(done.songs_found[1].title_hint, "Be Thou My Vision");
        assert!(done
            .error_detail
            .as_deref()
            .unwrap_or("")
            .contains("segmented 2"));
    }

    #[tokio::test]
    async fn process_pages_on_terminal_job_is_error() {
        let repo = repo().await;
        let job = repo.import("/x.pdf").await.unwrap();
        repo.set_status(&job.id, &SangbokJobStatus::Done, None, None)
            .await
            .unwrap();
        assert!(matches!(
            repo.process_pages(&job.id, &[]).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn set_status_updates_page_count() {
        let repo = repo().await;
        let job = repo.import("/big.pdf").await.unwrap();
        let updated = repo
            .set_status(&job.id, &SangbokJobStatus::Processing, None, Some(142))
            .await
            .unwrap();
        assert_eq!(updated.page_count, 142);
        assert_eq!(updated.status, "Processing");
    }
}
