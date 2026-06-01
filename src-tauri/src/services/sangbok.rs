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

    pub fn from_str(s: &str) -> AppResult<Self> {
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
        if SangbokJobStatus::from_str(&job.status)?.is_terminal() {
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
        if SangbokJobStatus::from_str(&job.status)?.is_terminal() {
            return Err(AppError::Validation(format!(
                "sangbok_job {id} cannot be processed from terminal state '{}'",
                job.status
            )));
        }

        let now = now_ms();
        // Advance to Processing.
        sqlx::query(
            "UPDATE sangbok_job SET status = 'Processing', updated_at = ? WHERE id = ?",
        )
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
            let back = SangbokJobStatus::from_str(s.as_str()).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn status_unknown_is_validation_error() {
        assert!(matches!(
            SangbokJobStatus::from_str("Running").unwrap_err(),
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
