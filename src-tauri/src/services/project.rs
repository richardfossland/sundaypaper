//! Project repository — the top-level container that groups documents.
//!
//! This is the reference repo for the data layer: every other entity follows
//! the same shape (owned `Db`, runtime-checked `query`/`query_as`, soft delete
//! via `deleted_at`, `Validation` on bad input, `NotFound` on missing rows).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A project: a named container grouping related documents and their work.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Project.ts")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
    /// Unix milliseconds, set when soft-deleted; `null` while live.
    pub deleted_at: Option<i64>,
}

pub struct ProjectRepo {
    db: Db,
}

impl ProjectRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Create a project. `name` is required (after trimming).
    pub async fn create(&self, name: &str, description: &str) -> AppResult<Project> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("project name is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO project (id, name, description, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live (non-deleted) project by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Project> {
        sqlx::query_as::<_, Project>("SELECT * FROM project WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "project",
                id: id.to_string(),
            })
    }

    /// All live projects, most-recently-updated first.
    pub async fn list(&self) -> AppResult<Vec<Project>> {
        let rows = sqlx::query_as::<_, Project>(
            "SELECT * FROM project WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Rename / re-describe a project. Returns the updated row.
    pub async fn update(&self, id: &str, name: &str, description: &str) -> AppResult<Project> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("project name is required".into()));
        }
        let affected = sqlx::query(
            "UPDATE project SET name = ?, description = ?, updated_at = ? \
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(description)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "project",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Soft-delete a project by stamping `deleted_at`. The row is kept (so it
    /// stays recoverable), which means its documents are NOT cascade-removed —
    /// they simply stop being reachable through `list`. A future hard-purge
    /// path will rely on the `ON DELETE CASCADE` FK to clean child rows.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected =
            sqlx::query("UPDATE project SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "project",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> ProjectRepo {
        ProjectRepo::new(Db::connect_memory().await.expect("connect"))
    }

    #[tokio::test]
    async fn create_get_roundtrip() {
        let repo = repo().await;
        let p = repo
            .create("  Easter 2026  ", "spring services")
            .await
            .unwrap();
        assert_eq!(p.name, "Easter 2026", "name is trimmed");
        assert_eq!(p.description, "spring services");
        assert!(p.deleted_at.is_none());
        let got = repo.get(&p.id).await.unwrap();
        assert_eq!(got, p);
    }

    #[tokio::test]
    async fn create_rejects_blank_name() {
        let repo = repo().await;
        let err = repo.create("   ", "").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn list_orders_by_updated_desc_and_hides_deleted() {
        let repo = repo().await;
        let a = repo.create("A", "").await.unwrap();
        let b = repo.create("B", "").await.unwrap();
        // Touch A so it becomes most-recently-updated.
        let a = repo.update(&a.id, "A2", "").await.unwrap();
        let ids: Vec<_> = repo
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, vec![a.id.clone(), b.id.clone()]);

        repo.delete(&b.id).await.unwrap();
        let ids: Vec<_> = repo
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, vec![a.id]);
    }

    #[tokio::test]
    async fn get_after_delete_is_not_found() {
        let repo = repo().await;
        let p = repo.create("Gone", "").await.unwrap();
        repo.delete(&p.id).await.unwrap();
        assert!(matches!(
            repo.get(&p.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
        // Deleting twice is also NotFound.
        assert!(matches!(
            repo.delete(&p.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn update_missing_is_not_found() {
        let repo = repo().await;
        assert!(matches!(
            repo.update("nope", "x", "").await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }
}
