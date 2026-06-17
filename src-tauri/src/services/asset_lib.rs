//! Asset Library — extended asset management with a typed `AssetKind` enum and
//! tag support.
//!
//! The existing `asset` service / table carries a free-text `kind` column.
//! Phase 1.3 adds two new columns via migration `0002_asset_lib.sql`:
//!
//!   - `asset_kind TEXT` — the canonical enum (Logo | Template | SongSheet |
//!     RecurringBlock | Font); defaults to `'Logo'` for rows created before
//!     the migration.
//!   - `tags TEXT` — comma-separated tag list (e.g. `"bulletin,2024"`); empty
//!     string means "no tags".
//!
//! `AssetLibRepo` operates on the same `asset` table but selects / inserts the
//! new columns too, returning `AssetLibEntry` instead of the base `Asset`.
//!
//! Opening a file (`asset_open`) is done via `tauri_plugin_opener` from the
//! command layer; here we just validate that the file exists.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

// ── AssetKind ────────────────────────────────────────────────────────────────

/// The canonical type of a library asset.
///
/// Serialises as the variant name (PascalCase) so the TypeScript side receives
/// `"Logo"`, `"Template"`, etc. — matching the discriminated union in the
/// frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/AssetKind.ts")]
pub enum AssetKind {
    /// Church / organisation logo, wordmark, or seal.
    Logo,
    /// Reusable document template (Typst source or Canva-style layout).
    Template,
    /// Song sheet — lyrics, chord chart, or music score.
    SongSheet,
    /// A block that recurs every Sunday (e.g. benediction, announcements header).
    RecurringBlock,
    /// A font file (TTF, OTF, WOFF2) for use in layouts.
    Font,
}

impl AssetKind {
    /// Canonical string used in the database column.
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetKind::Logo => "Logo",
            AssetKind::Template => "Template",
            AssetKind::SongSheet => "SongSheet",
            AssetKind::RecurringBlock => "RecurringBlock",
            AssetKind::Font => "Font",
        }
    }

    /// Parse from the database string. Unknown values fall back to `Logo` with
    /// a tracing warning so old/foreign rows don't crash the app.
    pub fn from_db(s: &str) -> Self {
        match s {
            "Logo" => AssetKind::Logo,
            "Template" => AssetKind::Template,
            "SongSheet" => AssetKind::SongSheet,
            "RecurringBlock" => AssetKind::RecurringBlock,
            "Font" => AssetKind::Font,
            other => {
                tracing::warn!(kind = other, "unknown asset_kind value, defaulting to Logo");
                AssetKind::Logo
            }
        }
    }
}

// ── AssetLibEntry ─────────────────────────────────────────────────────────────

/// A library asset with the full Phase 1.3 metadata. Returned by all
/// `AssetLibRepo` operations.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/AssetLibEntry.ts")]
pub struct AssetLibEntry {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    pub file_path: String,
    /// Comma-separated tag list — empty string means "no tags".
    pub tags: String,
    /// Unix milliseconds.
    pub created_at: i64,
}

// ── AssetLibEntry DB row ──────────────────────────────────────────────────────

/// sqlx projection row (columns returned by our SELECT statements).
#[derive(sqlx::FromRow)]
struct AssetRow {
    id: String,
    name: String,
    asset_kind: String,
    path: String,
    tags: String,
    created_at: i64,
}

impl From<AssetRow> for AssetLibEntry {
    fn from(r: AssetRow) -> Self {
        AssetLibEntry {
            id: r.id,
            name: r.name,
            kind: AssetKind::from_db(&r.asset_kind),
            file_path: r.path,
            tags: r.tags,
            created_at: r.created_at,
        }
    }
}

// ── AssetLibRepo ──────────────────────────────────────────────────────────────

pub struct AssetLibRepo {
    db: Db,
}

impl AssetLibRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Register a file in the asset library.
    ///
    /// Validates that `name` and `file_path` are non-empty. The `tags` string
    /// is stored as-is (the frontend is responsible for rendering it); pass an
    /// empty string for "no tags".
    pub async fn add(
        &self,
        name: &str,
        kind: AssetKind,
        file_path: &str,
        tags: &str,
    ) -> AppResult<AssetLibEntry> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("asset name is required".into()));
        }
        let file_path = file_path.trim();
        if file_path.is_empty() {
            return Err(AppError::Validation("asset file_path is required".into()));
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        let kind_str = kind.as_str();
        // The existing `kind` column (free-text) is set to the same value for
        // compatibility with the base AssetRepo.
        sqlx::query(
            "INSERT INTO asset \
                 (id, kind, asset_kind, name, path, tags, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(kind_str) // existing free-text `kind`
        .bind(kind_str) // new typed `asset_kind`
        .bind(name)
        .bind(file_path)
        .bind(tags)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live entry by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<AssetLibEntry> {
        sqlx::query_as::<_, AssetRow>(
            "SELECT id, name, asset_kind, path, tags, created_at \
             FROM asset WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.db.pool)
        .await?
        .map(AssetLibEntry::from)
        .ok_or_else(|| AppError::NotFound {
            entity: "asset",
            id: id.to_string(),
        })
    }

    /// List live assets, optionally filtered by `kind`. Newest first.
    pub async fn list(&self, kind: Option<AssetKind>) -> AppResult<Vec<AssetLibEntry>> {
        let rows = match kind {
            None => {
                sqlx::query_as::<_, AssetRow>(
                    "SELECT id, name, asset_kind, path, tags, created_at \
                     FROM asset WHERE deleted_at IS NULL ORDER BY created_at DESC",
                )
                .fetch_all(&self.db.pool)
                .await?
            }
            Some(k) => {
                sqlx::query_as::<_, AssetRow>(
                    "SELECT id, name, asset_kind, path, tags, created_at \
                     FROM asset WHERE asset_kind = ? AND deleted_at IS NULL \
                     ORDER BY created_at DESC",
                )
                .bind(k.as_str())
                .fetch_all(&self.db.pool)
                .await?
            }
        };
        Ok(rows.into_iter().map(AssetLibEntry::from).collect())
    }

    /// Soft-delete an asset.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected =
            sqlx::query("UPDATE asset SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(now_ms())
                .bind(id)
                .execute(&self.db.pool)
                .await?
                .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "asset",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    /// Verify that the file at `path` is accessible. Returns the path string
    /// on success so callers can hand it to `tauri_plugin_opener::open_path`.
    /// The actual OS `open` call is done in the command layer (requires Tauri
    /// app handle), not here, so this stays testable without Tauri.
    pub async fn path_for_open(&self, id: &str) -> AppResult<String> {
        let entry = self.get(id).await?;
        if !std::path::Path::new(&entry.file_path).exists() {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("asset file not found on disk: {}", entry.file_path),
            )));
        }
        Ok(entry.file_path)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::db::Db;

    async fn repo() -> AssetLibRepo {
        AssetLibRepo::new(Db::connect_memory().await.expect("in-memory db"))
    }

    // ── AssetKind round-trips ─────────────────────────────────────────────

    #[test]
    fn asset_kind_as_str_and_from_db_round_trip() {
        for kind in [
            AssetKind::Logo,
            AssetKind::Template,
            AssetKind::SongSheet,
            AssetKind::RecurringBlock,
            AssetKind::Font,
        ] {
            let s = kind.as_str();
            assert_eq!(
                AssetKind::from_db(s),
                kind,
                "round-trip failed for {kind:?}"
            );
        }
    }

    #[test]
    fn asset_kind_from_db_unknown_defaults_to_logo() {
        assert_eq!(AssetKind::from_db("Banana"), AssetKind::Logo);
    }

    // ── add ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_logo_and_retrieve_it() {
        let repo = repo().await;
        let entry = repo
            .add(
                "Church Logo",
                AssetKind::Logo,
                "/assets/logo.svg",
                "brand,svg",
            )
            .await
            .unwrap();
        assert_eq!(entry.name, "Church Logo");
        assert_eq!(entry.kind, AssetKind::Logo);
        assert_eq!(entry.file_path, "/assets/logo.svg");
        assert_eq!(entry.tags, "brand,svg");
    }

    #[tokio::test]
    async fn add_trims_name_whitespace() {
        let repo = repo().await;
        let entry = repo
            .add("  Song Sheet  ", AssetKind::SongSheet, "/sheet.pdf", "")
            .await
            .unwrap();
        assert_eq!(entry.name, "Song Sheet");
    }

    #[tokio::test]
    async fn add_rejects_empty_name() {
        let repo = repo().await;
        let err = repo
            .add("  ", AssetKind::Font, "/font.ttf", "")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_rejects_empty_path() {
        let repo = repo().await;
        let err = repo
            .add("My Font", AssetKind::Font, "  ", "")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_all_kinds_round_trip() {
        let repo = repo().await;
        for kind in [
            AssetKind::Logo,
            AssetKind::Template,
            AssetKind::SongSheet,
            AssetKind::RecurringBlock,
            AssetKind::Font,
        ] {
            let name = format!("Asset {:?}", kind);
            let entry = repo
                .add(&name, kind.clone(), "/some/path", "")
                .await
                .unwrap();
            assert_eq!(entry.kind, kind);
        }
    }

    // ── list ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_all_returns_everything() {
        let repo = repo().await;
        repo.add("Logo", AssetKind::Logo, "/logo.png", "")
            .await
            .unwrap();
        repo.add("Font", AssetKind::Font, "/font.ttf", "")
            .await
            .unwrap();
        repo.add("Sheet", AssetKind::SongSheet, "/sheet.pdf", "")
            .await
            .unwrap();
        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_filtered_by_kind() {
        let repo = repo().await;
        repo.add("Logo 1", AssetKind::Logo, "/logo1.png", "")
            .await
            .unwrap();
        repo.add("Logo 2", AssetKind::Logo, "/logo2.png", "")
            .await
            .unwrap();
        repo.add("Font", AssetKind::Font, "/font.ttf", "")
            .await
            .unwrap();
        let logos = repo.list(Some(AssetKind::Logo)).await.unwrap();
        assert_eq!(logos.len(), 2);
        assert!(logos.iter().all(|e| e.kind == AssetKind::Logo));
    }

    #[tokio::test]
    async fn list_filtered_returns_empty_when_no_match() {
        let repo = repo().await;
        repo.add("Logo", AssetKind::Logo, "/logo.png", "")
            .await
            .unwrap();
        let fonts = repo.list(Some(AssetKind::Font)).await.unwrap();
        assert!(fonts.is_empty());
    }

    #[tokio::test]
    async fn list_is_newest_first() {
        let repo = repo().await;
        let a = repo
            .add("A", AssetKind::Template, "/a.typ", "")
            .await
            .unwrap();
        let b = repo
            .add("B", AssetKind::Template, "/b.typ", "")
            .await
            .unwrap();
        // Pin distinct timestamps so the sort is deterministic.
        for (id, ts) in [(&a.id, 1_000_i64), (&b.id, 2_000_i64)] {
            sqlx::query("UPDATE asset SET created_at = ? WHERE id = ?")
                .bind(ts)
                .bind(id)
                .execute(&repo.db.pool)
                .await
                .unwrap();
        }
        let all = repo.list(None).await.unwrap();
        assert_eq!(all[0].id, b.id);
        assert_eq!(all[1].id, a.id);
    }

    // ── delete ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_hides_from_list() {
        let repo = repo().await;
        let entry = repo
            .add("Logo", AssetKind::Logo, "/logo.png", "")
            .await
            .unwrap();
        repo.delete(&entry.id).await.unwrap();
        assert!(repo.list(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_not_found_error() {
        let repo = repo().await;
        let err = repo.delete("no-such-id").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn double_delete_returns_not_found() {
        let repo = repo().await;
        let entry = repo
            .add("Font", AssetKind::Font, "/f.ttf", "")
            .await
            .unwrap();
        repo.delete(&entry.id).await.unwrap();
        let err = repo.delete(&entry.id).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    // ── path_for_open ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn path_for_open_returns_path_for_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("logo.png");
        std::fs::write(&file, b"fake-png").unwrap();
        let repo = repo().await;
        let entry = repo
            .add("Logo", AssetKind::Logo, file.to_str().unwrap(), "")
            .await
            .unwrap();
        let path = repo.path_for_open(&entry.id).await.unwrap();
        assert_eq!(path, file.to_str().unwrap());
    }

    #[tokio::test]
    async fn path_for_open_errors_when_file_missing() {
        let repo = repo().await;
        let entry = repo
            .add("Logo", AssetKind::Logo, "/does/not/exist.png", "")
            .await
            .unwrap();
        let err = repo.path_for_open(&entry.id).await.unwrap_err();
        assert!(matches!(err, AppError::Io(_)));
    }
}
