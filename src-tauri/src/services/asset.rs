//! Asset repository — files in the library (logo, image, font, scanned PDF).
//! Local-first: `path` is an absolute path on this device and `fingerprint` is
//! the O(1) content fingerprint used to relink a file that has moved (same
//! pattern as Verbatim/SundayStage). Soft-delete via `deleted_at`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

/// A file asset in the library.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, sqlx::FromRow)]
#[ts(export, export_to = "../../src/lib/bindings/Asset.ts")]
pub struct Asset {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub mime: Option<String>,
    pub byte_size: Option<i64>,
    pub fingerprint: Option<String>,
    /// Unix milliseconds.
    pub created_at: i64,
    /// Unix milliseconds.
    pub updated_at: i64,
    /// Unix milliseconds, set when soft-deleted; `null` while live.
    pub deleted_at: Option<i64>,
}

/// The mutable facts about an asset's backing file, gathered by the caller
/// (Phase 1.2 fills these from ffprobe/fingerprinting). Grouped so `create`
/// stays readable.
pub struct AssetInput<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub path: &'a str,
    pub mime: Option<&'a str>,
    pub byte_size: Option<i64>,
    pub fingerprint: Option<&'a str>,
}

/// The content-types the asset library accepts. The library holds document
/// media only — raster/vector images, PDFs, and font files — so an import
/// claiming any other type (an executable, a shell script, HTML, …) is refused
/// before it can be registered, later fed to the layout engine's `image()`
/// calls, or opened via `path_for_open`. A `None` mime is allowed (best-effort
/// import where the type is unknown); only an explicitly disallowed type is
/// rejected. Comparison is case-insensitive and ignores any `; charset=…`
/// parameter suffix.
fn is_allowed_mime(mime: &str) -> bool {
    let base = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    // Any image/* or font/* is a document asset; PDF is allowed explicitly.
    base.starts_with("image/")
        || base.starts_with("font/")
        || matches!(
            base.as_str(),
            "application/pdf"
                | "application/font-woff"
                | "application/font-woff2"
                | "application/x-font-ttf"
                | "application/x-font-otf"
                | "application/vnd.ms-opentype"
        )
}

pub struct AssetRepo {
    db: Db,
}

impl AssetRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Register a file in the library.
    pub async fn create(&self, input: AssetInput<'_>) -> AppResult<Asset> {
        if input.name.trim().is_empty() {
            return Err(AppError::Validation("asset name is required".into()));
        }
        if input.path.trim().is_empty() {
            return Err(AppError::Validation("asset path is required".into()));
        }
        if let Some(mime) = input.mime {
            if !is_allowed_mime(mime) {
                return Err(AppError::Validation(format!(
                    "unsupported asset type '{mime}'; only images, PDFs and fonts are allowed"
                )));
            }
        }
        let id = Uuid::now_v7().to_string();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO asset \
                 (id, kind, name, path, mime, byte_size, fingerprint, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.kind)
        .bind(input.name.trim())
        .bind(input.path)
        .bind(input.mime)
        .bind(input.byte_size)
        .bind(input.fingerprint)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;
        self.get(&id).await
    }

    /// Fetch a live asset by id, or `NotFound`.
    pub async fn get(&self, id: &str) -> AppResult<Asset> {
        sqlx::query_as::<_, Asset>("SELECT * FROM asset WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "asset",
                id: id.to_string(),
            })
    }

    /// All live assets, newest first.
    pub async fn list(&self) -> AppResult<Vec<Asset>> {
        let rows = sqlx::query_as::<_, Asset>(
            "SELECT * FROM asset WHERE deleted_at IS NULL ORDER BY created_at DESC",
        )
        .fetch_all(&self.db.pool)
        .await?;
        Ok(rows)
    }

    /// Find a live asset by its content fingerprint — the relink lookup for a
    /// file whose path changed. Returns the first match (newest first).
    pub async fn find_by_fingerprint(&self, fingerprint: &str) -> AppResult<Option<Asset>> {
        let row = sqlx::query_as::<_, Asset>(
            "SELECT * FROM asset WHERE fingerprint = ? AND deleted_at IS NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.db.pool)
        .await?;
        Ok(row)
    }

    /// Point an asset at a new file location (relink).
    pub async fn relink(&self, id: &str, path: &str) -> AppResult<Asset> {
        if path.trim().is_empty() {
            return Err(AppError::Validation("asset path is required".into()));
        }
        let affected = sqlx::query(
            "UPDATE asset SET path = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(path)
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
        self.get(id).await
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
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> AssetRepo {
        AssetRepo::new(Db::connect_memory().await.expect("connect"))
    }

    fn logo<'a>() -> AssetInput<'a> {
        AssetInput {
            kind: "logo",
            name: "Church logo",
            path: "/library/logo.png",
            mime: Some("image/png"),
            byte_size: Some(2048),
            fingerprint: Some("fp-abc"),
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let repo = repo().await;
        let a = repo.create(logo()).await.unwrap();
        assert_eq!(a.kind, "logo");
        assert_eq!(a.byte_size, Some(2048));
        assert_eq!(repo.get(&a.id).await.unwrap(), a);
    }

    #[tokio::test]
    async fn relink_by_fingerprint() {
        let repo = repo().await;
        let a = repo.create(logo()).await.unwrap();
        let found = repo.find_by_fingerprint("fp-abc").await.unwrap().unwrap();
        assert_eq!(found.id, a.id);
        let relinked = repo.relink(&a.id, "/moved/logo.png").await.unwrap();
        assert_eq!(relinked.path, "/moved/logo.png");
        assert!(repo.find_by_fingerprint("fp-none").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn soft_delete_hides_from_list_and_lookup() {
        let repo = repo().await;
        let a = repo.create(logo()).await.unwrap();
        repo.delete(&a.id).await.unwrap();
        assert!(repo.list().await.unwrap().is_empty());
        assert!(repo.find_by_fingerprint("fp-abc").await.unwrap().is_none());
        assert!(matches!(
            repo.get(&a.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn create_rejects_disallowed_mime_type() {
        // Prove-first (security): the asset library should only accept document
        // media — images, PDFs, fonts. An import claiming an executable / script
        // / HTML content-type must be refused, not silently registered (these
        // would later feed the layout engine's `image()` calls and the
        // `path_for_open` "open this file" flow).
        let repo = repo().await;
        for bad in [
            "application/x-msdownload",
            "application/x-sh",
            "text/html",
            "application/javascript",
        ] {
            let mut input = logo();
            input.mime = Some(bad);
            let err = repo.create(input).await.unwrap_err();
            assert!(
                matches!(err, AppError::Validation(_)),
                "expected disallowed mime {bad:?} to be rejected"
            );
        }
    }

    #[tokio::test]
    async fn create_accepts_allowed_document_mime_types() {
        let repo = repo().await;
        for ok in [
            "image/png",
            "image/jpeg",
            "image/svg+xml",
            "application/pdf",
            "font/ttf",
            "font/otf",
        ] {
            let mut input = logo();
            input.mime = Some(ok);
            assert!(
                repo.create(input).await.is_ok(),
                "expected allowed mime {ok:?} to be accepted"
            );
        }
    }

    #[tokio::test]
    async fn create_accepts_absent_mime() {
        // A missing content-type is allowed (best-effort import); only an
        // explicitly disallowed type is rejected.
        let repo = repo().await;
        let mut input = logo();
        input.mime = None;
        assert!(repo.create(input).await.is_ok());
    }

    #[tokio::test]
    async fn create_requires_name_and_path() {
        let repo = repo().await;
        let mut bad = logo();
        bad.name = "  ";
        assert!(matches!(
            repo.create(bad).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
