//! SQLite data layer — the local-first store every repository sits on.
//!
//! `Db` wraps an `sqlx` connection pool. On connect it runs the embedded
//! migrations in `sql/` and enables foreign-key enforcement on every
//! connection. Queries are **runtime-checked** (`query` / `query_as`), so no
//! `DATABASE_URL` or `.sqlx` cache is needed at compile time — the schema lives
//! in `sql/` and is verified by the migration run + the repo unit tests.
//!
//! `Db` is cheap to `Clone` (the pool is an `Arc` internally), so repositories
//! take an owned `Db` rather than a borrow — this keeps them free of lifetimes
//! across `.await` points in async Tauri commands.

use std::path::Path;
#[cfg(test)]
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::error::AppResult;

/// Handle to the SQLite store. Hold one in `AppState`; clone it into repos.
#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    /// Open (creating if absent) the database file at `path`, run migrations,
    /// and enforce foreign keys. Used by the running app.
    pub async fn connect_file(path: &Path) -> AppResult<Self> {
        let opts = SqliteConnectOptions::new().filename(path);
        Self::connect_with(opts, 5).await
    }

    /// Open a private in-memory database for tests. A single pooled connection
    /// keeps the `:memory:` instance alive for the whole test.
    #[cfg(test)]
    pub async fn connect_memory() -> AppResult<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("static sqlite memory url parses");
        Self::connect_with(opts, 1).await
    }

    async fn connect_with(opts: SqliteConnectOptions, max_connections: u32) -> AppResult<Self> {
        let opts = opts.create_if_missing(true).foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(opts)
            .await?;
        // Migrations are embedded at compile time from the repo-root `sql/` dir
        // (relative to this crate's manifest). `run` is idempotent — applied
        // versions are tracked in `_sqlx_migrations`.
        sqlx::migrate!("../sql").run(&pool).await?;
        Ok(Self { pool })
    }
}

/// Current wall-clock time as unix milliseconds — the project's timestamp unit.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_and_tables_exist() {
        let db = Db::connect_memory().await.expect("connect");
        // Every entity table from 0001_init must be present after migration.
        for table in [
            "project",
            "template",
            "asset",
            "song",
            "document",
            "block",
            "import_job",
            "setting",
            "doc_template",
            "template_var",
            "sangbok_job",
            "song_extract",
        ] {
            let found: Option<String> =
                sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
                    .bind(table)
                    .fetch_optional(&db.pool)
                    .await
                    .expect("query sqlite_master");
            assert_eq!(found.as_deref(), Some(table), "missing table {table}");
        }
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let db = Db::connect_memory().await.expect("connect");
        // Inserting a document for a non-existent project must be rejected.
        let now = now_ms();
        let res = sqlx::query(
            "INSERT INTO document (id, project_id, title, kind, page_size, position, created_at, updated_at) \
             VALUES ('d1', 'no-such-project', 'X', 'program', 'A4', 0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&db.pool)
        .await;
        assert!(res.is_err(), "FK violation should be rejected");
    }
}
