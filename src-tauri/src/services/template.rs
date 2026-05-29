//! Template repository — reusable layout templates. `source` holds Typst source
//! (empty until the layout engine lands in Phase 4.2). Soft-delete via
//! `deleted_at`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A reusable layout template.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Template.ts")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// Typst source; empty until Phase 4.2.
    pub source: String,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
    /// Unix milliseconds, set when soft-deleted; `null` while live.
    pub deleted_at: Option<i64>,
}

pub struct TemplateRepo {
    db: Db,
}

impl TemplateRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Create a template. `name` and `kind` are required.
    pub async fn create(&self, name: &str, kind: &str, source: &str) -> AppResult<Template> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("template name is required".into()));
        }
        if kind.trim().is_empty() {
            return Err(AppError::Validation("template kind is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO template (id, name, kind, source, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(kind)
        .bind(source)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live template by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Template> {
        sqlx::query_as::<_, Template>("SELECT * FROM template WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "template",
                id: id.to_string(),
            })
    }

    /// All live templates, alphabetised by name.
    pub async fn list(&self) -> AppResult<Vec<Template>> {
        let rows = sqlx::query_as::<_, Template>(
            "SELECT * FROM template WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE ASC",
        )
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Update a template's editable fields.
    pub async fn update(
        &self,
        id: &str,
        name: &str,
        kind: &str,
        source: &str,
    ) -> AppResult<Template> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("template name is required".into()));
        }
        let affected = sqlx::query(
            "UPDATE template SET name = ?, kind = ?, source = ?, updated_at = ? \
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(kind)
        .bind(source)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "template",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Soft-delete a template. Documents bound to it keep their row; the FK is
    /// `ON DELETE SET NULL` for a future hard purge.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected =
            sqlx::query("UPDATE template SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "template",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> TemplateRepo {
        TemplateRepo::new(Db::connect_memory().await.expect("connect"))
    }

    #[tokio::test]
    async fn create_get_update_delete() {
        let repo = repo().await;
        let t = repo.create("  Bulletin  ", "program", "").await.unwrap();
        assert_eq!(t.name, "Bulletin");
        let t = repo
            .update(&t.id, "Bulletin v2", "program", "#set page(...)")
            .await
            .unwrap();
        assert_eq!(t.source, "#set page(...)");
        repo.delete(&t.id).await.unwrap();
        assert!(repo.list().await.unwrap().is_empty());
        assert!(matches!(
            repo.get(&t.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn create_requires_name_and_kind() {
        let repo = repo().await;
        assert!(matches!(
            repo.create("  ", "program", "").await.unwrap_err(),
            AppError::Validation(_)
        ));
        assert!(matches!(
            repo.create("X", "  ", "").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
