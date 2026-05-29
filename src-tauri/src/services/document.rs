//! Document repository — a document belongs to a project and is the thing a
//! block tree renders into a PDF. Demonstrates the parent-child repo pattern
//! (FK to `project`, `list_by_project`, append-ordering via `position`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A document within a project. `kind` selects the document family (program,
/// song_sheet, magazine, poster, form); `position` orders it within the project.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Document.ts")]
pub struct Document {
    pub id: String,
    pub project_id: String,
    pub template_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub page_size: String,
    pub position: i64,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
    /// Unix milliseconds, set when soft-deleted; `null` while live.
    pub deleted_at: Option<i64>,
}

pub struct DocumentRepo {
    db: Db,
}

impl DocumentRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Create a document in `project_id`, appended after existing ones. Fails
    /// with a foreign-key `Database` error if the project does not exist.
    pub async fn create(
        &self,
        project_id: &str,
        title: &str,
        kind: &str,
        page_size: &str,
    ) -> AppResult<Document> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::Validation("document title is required".into()));
        }
        if kind.trim().is_empty() {
            return Err(AppError::Validation("document kind is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        let position = self.next_position(project_id).await?;
        sqlx::query(
            "INSERT INTO document \
                 (id, project_id, title, kind, page_size, position, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(title)
        .bind(kind)
        .bind(page_size)
        .bind(position)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live document by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Document> {
        sqlx::query_as::<_, Document>("SELECT * FROM document WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "document",
                id: id.to_string(),
            })
    }

    /// All live documents in a project, in `position` order.
    pub async fn list_by_project(&self, project_id: &str) -> AppResult<Vec<Document>> {
        let rows = sqlx::query_as::<_, Document>(
            "SELECT * FROM document WHERE project_id = ? AND deleted_at IS NULL \
             ORDER BY position ASC, created_at ASC",
        )
        .bind(project_id)
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Update the editable header fields of a document.
    pub async fn update(
        &self,
        id: &str,
        title: &str,
        kind: &str,
        page_size: &str,
    ) -> AppResult<Document> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::Validation("document title is required".into()));
        }
        let affected = sqlx::query(
            "UPDATE document SET title = ?, kind = ?, page_size = ?, updated_at = ? \
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(title)
        .bind(kind)
        .bind(page_size)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "document",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Soft-delete a document.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected =
            sqlx::query("UPDATE document SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "document",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    /// Next append position for a project = max live position + 1 (0 if none).
    async fn next_position(&self, project_id: &str) -> AppResult<i64> {
        let max: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(position) FROM document WHERE project_id = ? AND deleted_at IS NULL",
        )
        .bind(project_id)
        .fetch_one(&self.db.pool)
        .await?;
        Ok(max.map(|m| m + 1).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::project::ProjectRepo;

    /// A repo pair sharing one in-memory db, plus a project to hang docs on.
    async fn fixture() -> (DocumentRepo, String) {
        let db = Db::connect_memory().await.expect("connect");
        let project = ProjectRepo::new(db.clone())
            .create("Sunday", "")
            .await
            .expect("project");
        (DocumentRepo::new(db), project.id)
    }

    #[tokio::test]
    async fn create_appends_positions_in_order() {
        let (docs, pid) = fixture().await;
        let a = docs.create(&pid, "Program", "program", "A4").await.unwrap();
        let b = docs
            .create(&pid, "Song sheet", "song_sheet", "A4")
            .await
            .unwrap();
        assert_eq!(a.position, 0);
        assert_eq!(b.position, 1);
        let listed: Vec<_> = docs
            .list_by_project(&pid)
            .await
            .unwrap()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(listed, vec![a.id, b.id]);
    }

    #[tokio::test]
    async fn create_for_unknown_project_violates_fk() {
        let (docs, _pid) = fixture().await;
        let err = docs
            .create("no-such-project", "X", "program", "A4")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Database(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn create_validates_title_and_kind() {
        let (docs, pid) = fixture().await;
        assert!(matches!(
            docs.create(&pid, "  ", "program", "A4").await.unwrap_err(),
            AppError::Validation(_)
        ));
        assert!(matches!(
            docs.create(&pid, "Title", "  ", "A4").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn update_and_soft_delete() {
        let (docs, pid) = fixture().await;
        let d = docs.create(&pid, "Draft", "program", "A4").await.unwrap();
        let d = docs.update(&d.id, "Final", "program", "A5").await.unwrap();
        assert_eq!(d.title, "Final");
        assert_eq!(d.page_size, "A5");
        docs.delete(&d.id).await.unwrap();
        assert!(docs.list_by_project(&pid).await.unwrap().is_empty());
        assert!(matches!(
            docs.get(&d.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }
}
