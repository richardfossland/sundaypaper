//! Centralised error type for the SundayPaper backend.
//!
//! Tauri commands return `Result<T, AppError>` — `AppError` implements
//! `serde::Serialize` so it crosses the IPC boundary as a stable JSON shape
//! (`{ code, message }`) that the renderer can pattern-match on.
//!
//! Phase 0 keeps this lean. Later phases add data-layer variants (e.g.
//! `Database`, `Migration` once sqlx lands in Phase 1) and domain variants
//! (`PdfParse`, `OcrFailed`, ...). Keep `code()` and the TS `AppError` union in
//! `src/lib/bindings/index.ts` in sync when you add a variant.

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    /// Entity not found by ID — distinct from a general error so the renderer
    /// can render a "404" UI.
    #[error("not found: {entity} id={id}")]
    NotFound { entity: &'static str, id: String },

    /// Caller passed input that fails our domain rules.
    #[error("validation: {0}")]
    Validation(String),

    /// File-system / IO failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialisation/deserialisation issue.
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),

    /// A query against the SQLite store failed.
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),

    /// A schema migration failed to apply.
    #[error("migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// A PDF read / render / manipulation operation failed.
    #[error("pdf: {0}")]
    Pdf(String),

    /// The operation needs an optional cargo feature that this build was
    /// compiled without (e.g. `pdf`). The renderer can surface a clear
    /// "this build can't do that" message instead of a generic failure.
    #[error("feature '{feature}' is not enabled in this build")]
    FeatureDisabled { feature: &'static str },

    /// Anything else we couldn't classify.
    #[error("internal: {0}")]
    Internal(String),
}

impl AppError {
    /// Short, machine-readable category for the renderer to switch on.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotFound { .. } => "not_found",
            AppError::Validation(_) => "validation",
            AppError::Io(_) => "io",
            AppError::Json(_) => "json",
            AppError::Database(_) => "database",
            AppError::Migration(_) => "migration",
            AppError::Pdf(_) => "pdf",
            AppError::FeatureDisabled { .. } => "feature_disabled",
            AppError::Internal(_) => "internal",
        }
    }
}

/// Custom serializer so the JSON sent to the renderer has both a stable
/// `code` field (for switch statements) and the human-readable `message`.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

/// Convenience alias for the project.
pub type AppResult<T> = Result<T, AppError>;
