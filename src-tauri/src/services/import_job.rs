//! Import-job repository — the backward direction: a split / OCR / merge job on
//! an existing PDF (Phase 1.2+). This is a job log, not library content, so it
//! has no soft-delete; rows are kept for history and updated as the job runs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A backward-direction ingest job.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/ImportJob.ts")]
pub struct ImportJob {
    pub id: String,
    /// Owning project, or `null` for a standalone job.
    pub project_id: Option<String>,
    pub source_path: String,
    pub kind: String,
    /// One of: pending | running | done | error.
    pub status: String,
    /// Free-form detail / error message.
    pub detail: Option<String>,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
}

/// The lifecycle states a job moves through.
const STATUSES: [&str; 4] = ["pending", "running", "done", "error"];

pub struct ImportJobRepo {
    db: Db,
}

impl ImportJobRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Queue a new job in the `pending` state.
    pub async fn create(
        &self,
        project_id: Option<&str>,
        source_path: &str,
        kind: &str,
    ) -> AppResult<ImportJob> {
        if source_path.trim().is_empty() {
            return Err(AppError::Validation(
                "import source_path is required".into(),
            ));
        }
        if kind.trim().is_empty() {
            return Err(AppError::Validation("import kind is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO import_job \
                 (id, project_id, source_path, kind, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'pending', ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(source_path)
        .bind(kind)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a job by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<ImportJob> {
        sqlx::query_as::<_, ImportJob>("SELECT * FROM import_job WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "import_job",
                id: id.to_string(),
            })
    }

    /// All jobs, newest first. `id DESC` is a stable tiebreaker: UUID v7 is
    /// time-ordered, so within the same `created_at` millisecond the later id
    /// still sorts first — making the order deterministic instead of arbitrary.
    pub async fn list(&self) -> AppResult<Vec<ImportJob>> {
        let rows = sqlx::query_as::<_, ImportJob>(
            "SELECT * FROM import_job ORDER BY created_at DESC, id DESC",
        )
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Advance a job's status and optional detail. Rejects unknown statuses.
    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        detail: Option<&str>,
    ) -> AppResult<ImportJob> {
        if !STATUSES.contains(&status) {
            return Err(AppError::Validation(format!(
                "unknown import status '{status}' (expected one of {STATUSES:?})"
            )));
        }
        let affected = sqlx::query(
            "UPDATE import_job SET status = ?, detail = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(detail)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "import_job",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> ImportJobRepo {
        ImportJobRepo::new(Db::connect_memory().await.expect("connect"))
    }

    #[tokio::test]
    async fn create_defaults_to_pending_then_advances() {
        let repo = repo().await;
        let job = repo.create(None, "/in/scan.pdf", "ocr").await.unwrap();
        assert_eq!(job.status, "pending");
        let job = repo
            .update_status(&job.id, "done", Some("12 pages"))
            .await
            .unwrap();
        assert_eq!(job.status, "done");
        assert_eq!(job.detail.as_deref(), Some("12 pages"));
    }

    #[tokio::test]
    async fn unknown_status_is_rejected() {
        let repo = repo().await;
        let job = repo.create(None, "/in/scan.pdf", "split").await.unwrap();
        assert!(matches!(
            repo.update_status(&job.id, "exploded", None)
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn list_is_newest_first() {
        let repo = repo().await;
        let a = repo.create(None, "/a.pdf", "merge").await.unwrap();
        let b = repo.create(None, "/b.pdf", "merge").await.unwrap();
        // Stamp distinct `created_at` values so the primary sort key alone
        // decides the order — `create` uses `now_ms()` and both rows can land in
        // the same millisecond, which is exactly the tie the query now breaks on
        // `id DESC`. Pinning the timestamps keeps this test's intent (newest
        // first by time) crisp and independent of that tiebreaker.
        for (id, ts) in [(&a.id, 1_000_i64), (&b.id, 2_000_i64)] {
            sqlx::query("UPDATE import_job SET created_at = ? WHERE id = ?")
                .bind(ts)
                .bind(id)
                .execute(&repo.db.pool)
                .await
                .unwrap();
        }
        let ids: Vec<_> = repo
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|j| j.id)
            .collect();
        assert_eq!(ids, vec![b.id, a.id]);
    }

    #[tokio::test]
    async fn create_validates_inputs() {
        let repo = repo().await;
        assert!(matches!(
            repo.create(None, "  ", "ocr").await.unwrap_err(),
            AppError::Validation(_)
        ));
        assert!(matches!(
            repo.create(None, "/x.pdf", "  ").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
