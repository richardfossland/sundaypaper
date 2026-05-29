//! Song repository — the song catalog. `tono_work_id` is first-class from day
//! one so songs flowing to SundaySong carry their Nordic rights id (Phase 8).
//! Soft-delete via `deleted_at`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A song in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Song.ts")]
pub struct Song {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub body: String,
    pub language: Option<String>,
    pub tono_work_id: Option<String>,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
    /// Unix milliseconds, set when soft-deleted; `null` while live.
    pub deleted_at: Option<i64>,
}

/// Editable fields of a song, grouped so create/update stay readable.
pub struct SongInput<'a> {
    pub title: &'a str,
    pub author: Option<&'a str>,
    pub body: &'a str,
    pub language: Option<&'a str>,
    pub tono_work_id: Option<&'a str>,
}

pub struct SongRepo {
    db: Db,
}

impl SongRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Add a song to the catalog. `title` is required.
    pub async fn create(&self, input: SongInput<'_>) -> AppResult<Song> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::Validation("song title is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO song \
                 (id, title, author, body, language, tono_work_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(title)
        .bind(input.author)
        .bind(input.body)
        .bind(input.language)
        .bind(input.tono_work_id)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live song by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Song> {
        sqlx::query_as::<_, Song>("SELECT * FROM song WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "song",
                id: id.to_string(),
            })
    }

    /// All live songs, alphabetised by title.
    pub async fn list(&self) -> AppResult<Vec<Song>> {
        let rows = sqlx::query_as::<_, Song>(
            "SELECT * FROM song WHERE deleted_at IS NULL ORDER BY title COLLATE NOCASE ASC",
        )
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Update a song's editable fields.
    pub async fn update(&self, id: &str, input: SongInput<'_>) -> AppResult<Song> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::Validation("song title is required".into()));
        }
        let affected = sqlx::query(
            "UPDATE song SET title = ?, author = ?, body = ?, language = ?, tono_work_id = ?, \
                 updated_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(title)
        .bind(input.author)
        .bind(input.body)
        .bind(input.language)
        .bind(input.tono_work_id)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "song",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    /// Soft-delete a song.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected =
            sqlx::query("UPDATE song SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "song",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> SongRepo {
        SongRepo::new(Db::connect_memory().await.expect("connect"))
    }

    fn input<'a>(title: &'a str) -> SongInput<'a> {
        SongInput {
            title,
            author: Some("Trad."),
            body: "verse 1",
            language: Some("no"),
            tono_work_id: Some("TONO-123"),
        }
    }

    #[tokio::test]
    async fn create_preserves_tono_work_id() {
        let repo = repo().await;
        let s = repo.create(input("  Deilig er jorden  ")).await.unwrap();
        assert_eq!(s.title, "Deilig er jorden", "title trimmed");
        assert_eq!(s.tono_work_id.as_deref(), Some("TONO-123"));
        assert_eq!(repo.get(&s.id).await.unwrap(), s);
    }

    #[tokio::test]
    async fn list_is_alphabetical_case_insensitive() {
        let repo = repo().await;
        repo.create(input("banana")).await.unwrap();
        repo.create(input("Apple")).await.unwrap();
        let titles: Vec<_> = repo
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(titles, vec!["Apple", "banana"]);
    }

    #[tokio::test]
    async fn update_and_delete() {
        let repo = repo().await;
        let s = repo.create(input("Old")).await.unwrap();
        let mut next = input("New");
        next.tono_work_id = None;
        let s = repo.update(&s.id, next).await.unwrap();
        assert_eq!(s.title, "New");
        assert!(s.tono_work_id.is_none());
        repo.delete(&s.id).await.unwrap();
        assert!(matches!(
            repo.get(&s.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn create_requires_title() {
        let repo = repo().await;
        assert!(matches!(
            repo.create(input("   ")).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
