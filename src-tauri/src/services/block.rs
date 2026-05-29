//! Block repository — a node in a document's block tree. `parent_id` NULL means
//! top level; `position` orders siblings within their parent. `data` is a
//! kind-specific JSON payload (validated as JSON, not against a per-kind schema
//! yet). Blocks have no soft-delete: deleting a block hard-deletes it and, via
//! the `ON DELETE CASCADE` self-FK, its whole subtree.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A node in a document's block tree.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Block.ts")]
pub struct Block {
    pub id: String,
    pub document_id: String,
    /// Parent block id; `null` for a top-level block.
    pub parent_id: Option<String>,
    pub kind: String,
    pub position: i64,
    /// Kind-specific payload as a JSON string.
    pub data: String,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
}

pub struct BlockRepo {
    db: Db,
}

impl BlockRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Insert a block, appended after its siblings (same document + parent).
    /// `data` must be valid JSON (defaults to `{}` when empty).
    pub async fn create(
        &self,
        document_id: &str,
        parent_id: Option<&str>,
        kind: &str,
        data: &str,
    ) -> AppResult<Block> {
        if kind.trim().is_empty() {
            return Err(AppError::Validation("block kind is required".into()));
        }
        let data = normalise_json(data)?;
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        let position = self.next_position(document_id, parent_id).await?;
        sqlx::query(
            "INSERT INTO block \
                 (id, document_id, parent_id, kind, position, data, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(document_id)
        .bind(parent_id)
        .bind(kind)
        .bind(position)
        .bind(&data)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a block by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Block> {
        sqlx::query_as::<_, Block>("SELECT * FROM block WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "block",
                id: id.to_string(),
            })
    }

    /// All blocks in a document, flat, ordered by `position`. The caller builds
    /// the tree by grouping on `parent_id`.
    pub async fn list_by_document(&self, document_id: &str) -> AppResult<Vec<Block>> {
        let rows = sqlx::query_as::<_, Block>(
            "SELECT * FROM block WHERE document_id = ? ORDER BY position ASC, created_at ASC",
        )
        .bind(document_id)
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Update a block's kind and/or payload. `data` must be valid JSON.
    pub async fn update(&self, id: &str, kind: &str, data: &str) -> AppResult<Block> {
        if kind.trim().is_empty() {
            return Err(AppError::Validation("block kind is required".into()));
        }
        let data = normalise_json(data)?;
        let affected =
            sqlx::query("UPDATE block SET kind = ?, data = ?, updated_at = ? WHERE id = ?")
                .bind(kind)
                .bind(&data)
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "block",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Hard-delete a block and its subtree (via the self-referential cascade).
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected = sqlx::query("DELETE FROM block WHERE id = ?")
            .bind(id)
            .execute(&self.db.pool)
            .await?
            .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "block",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    /// Next append position within a sibling group. `parent_id IS ?` is
    /// null-safe in SQLite, so it matches top-level blocks when `parent_id` is
    /// NULL and a specific parent otherwise.
    async fn next_position(&self, document_id: &str, parent_id: Option<&str>) -> AppResult<i64> {
        let max: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(position) FROM block WHERE document_id = ? AND parent_id IS ?",
        )
        .bind(document_id)
        .bind(parent_id)
        .fetch_one(&self.db.pool)
        .await?;
        Ok(max.map(|m| m + 1).unwrap_or(0))
    }
}

/// Validate that `data` parses as JSON; an empty string normalises to `{}`.
fn normalise_json(data: &str) -> AppResult<String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| AppError::Validation(format!("block data must be valid JSON: {e}")))?;
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{document::DocumentRepo, project::ProjectRepo};

    /// One in-memory db with a project + document to hang blocks on.
    async fn fixture() -> (BlockRepo, String) {
        let db = Db::connect_memory().await.expect("connect");
        let project = ProjectRepo::new(db.clone())
            .create("P", "")
            .await
            .expect("project");
        let doc = DocumentRepo::new(db.clone())
            .create(&project.id, "Doc", "program", "A4")
            .await
            .expect("document");
        (BlockRepo::new(db), doc.id)
    }

    #[tokio::test]
    async fn positions_are_scoped_to_sibling_group() {
        let (blocks, doc) = fixture().await;
        let root_a = blocks.create(&doc, None, "liturgy", "").await.unwrap();
        let root_b = blocks.create(&doc, None, "song", "").await.unwrap();
        // Children of root_a get their own position space starting at 0.
        let child = blocks
            .create(&doc, Some(&root_a.id), "scripture", "")
            .await
            .unwrap();
        assert_eq!(root_a.position, 0);
        assert_eq!(root_b.position, 1);
        assert_eq!(child.position, 0, "child positions restart per parent");
        assert_eq!(child.data, "{}", "empty data normalises to {{}}");
        assert_eq!(blocks.list_by_document(&doc).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn invalid_json_data_is_rejected() {
        let (blocks, doc) = fixture().await;
        let err = blocks
            .create(&doc, None, "image", "{not json")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn delete_cascades_to_subtree() {
        let (blocks, doc) = fixture().await;
        let parent = blocks.create(&doc, None, "liturgy", "").await.unwrap();
        let _child = blocks
            .create(&doc, Some(&parent.id), "scripture", "")
            .await
            .unwrap();
        blocks.delete(&parent.id).await.unwrap();
        assert!(blocks.list_by_document(&doc).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn update_changes_kind_and_data() {
        let (blocks, doc) = fixture().await;
        let b = blocks.create(&doc, None, "announcement", "").await.unwrap();
        let b = blocks
            .update(&b.id, "announcement", r#"{"text":"hi"}"#)
            .await
            .unwrap();
        assert_eq!(b.data, r#"{"text":"hi"}"#);
    }
}
