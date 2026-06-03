//! Setting repository — a simple local-only key/value store (locale, last
//! project, ... ). Keys are unique; `set` upserts. No soft-delete: a setting is
//! either present or removed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A single key/value setting.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Setting.ts")]
pub struct Setting {
    pub key: String,
    pub value: String,
    /// Unix milliseconds.
    pub updated_at: i64,
}

pub struct SettingRepo {
    db: Db,
}

impl SettingRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Read a setting's value, or `None` if unset.
    pub async fn get(&self, key: &str) -> AppResult<Option<String>> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM setting WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.db.pool)
            .await?;
        Ok(value)
    }

    /// Insert or overwrite a setting. `key` is required.
    pub async fn set(&self, key: &str, value: &str) -> AppResult<Setting> {
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::Validation("setting key is required".into()));
        }
        let now = now_ms();
        sqlx::query(
            "INSERT INTO setting (key, value, updated_at) VALUES (?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        Ok(Setting {
            key: key.to_string(),
            value: value.to_string(),
            updated_at: now,
        })
    }

    /// All settings, ordered by key.
    pub async fn list(&self) -> AppResult<Vec<Setting>> {
        let rows = sqlx::query_as::<_, Setting>("SELECT * FROM setting ORDER BY key ASC")
            .fetch_all(&self.db.pool)
            .await?;
        Ok(rows)
    }

    /// Remove a setting. Returns `NotFound` if the key was not set.
    pub async fn delete(&self, key: &str) -> AppResult<()> {
        let affected = sqlx::query("DELETE FROM setting WHERE key = ?")
            .bind(key)
            .execute(&self.db.pool)
            .await?
            .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "setting",
                id: key.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> SettingRepo {
        SettingRepo::new(Db::connect_memory().await.expect("connect"))
    }

    #[tokio::test]
    async fn set_is_upsert() {
        let repo = repo().await;
        assert!(repo.get("locale").await.unwrap().is_none());
        repo.set("locale", "no").await.unwrap();
        assert_eq!(repo.get("locale").await.unwrap().as_deref(), Some("no"));
        // Setting the same key again overwrites rather than erroring.
        let s = repo.set("  locale  ", "en").await.unwrap();
        assert_eq!(s.key, "locale", "key is trimmed");
        assert_eq!(repo.get("locale").await.unwrap().as_deref(), Some("en"));
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn delete_missing_is_not_found() {
        let repo = repo().await;
        repo.set("a", "1").await.unwrap();
        repo.delete("a").await.unwrap();
        assert!(repo.get("a").await.unwrap().is_none());
        assert!(matches!(
            repo.delete("a").await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn full_roundtrip_get_set_list_delete() {
        // Mirrors the Settings-page flow: read (empty) → set a couple keys →
        // list them back ordered → delete → confirm gone.
        let repo = repo().await;
        assert!(repo.get("locale").await.unwrap().is_none());

        repo.set("locale", "no").await.unwrap();
        let saved = repo.set("anthropic_api_key", "sk-ant-x").await.unwrap();
        assert_eq!(saved.value, "sk-ant-x");

        // Read-back via get.
        assert_eq!(repo.get("locale").await.unwrap().as_deref(), Some("no"));

        // list() is ordered by key ascending.
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].key, "anthropic_api_key");
        assert_eq!(all[1].key, "locale");

        // Deleting one leaves the other intact.
        repo.delete("anthropic_api_key").await.unwrap();
        assert!(repo.get("anthropic_api_key").await.unwrap().is_none());
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn set_requires_key() {
        let repo = repo().await;
        assert!(matches!(
            repo.set("   ", "x").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
