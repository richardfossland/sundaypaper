//! Asset Library IPC commands (Phase 1.3).
//!
//! These commands expose the typed `AssetKind` + tags layer from
//! `services::asset_lib`. They complement (not replace) the existing
//! `commands::asset` layer; the frontend can migrate progressively.
//!
//! `asset_open` uses `tauri_plugin_opener` to hand the file to the OS default
//! application — the only place in the backend that needs the Tauri app handle
//! for this feature.

use tauri::State;

use crate::error::AppResult;
use crate::services::asset_lib::{AssetKind, AssetLibEntry, AssetLibRepo};
use crate::AppState;

/// Register a file in the asset library.
///
/// `kind` must be one of: `"Logo"`, `"Template"`, `"SongSheet"`,
/// `"RecurringBlock"`, `"Font"`. Unknown values are rejected with a
/// `validation` error so the frontend never silently creates an untyped entry.
#[tauri::command]
pub async fn asset_add(
    state: State<'_, AppState>,
    name: String,
    kind: String,
    file_path: String,
    tags: Option<String>,
) -> AppResult<AssetLibEntry> {
    let kind = parse_kind(&kind)?;
    AssetLibRepo::new(state.db.clone())
        .add(&name, kind, &file_path, tags.as_deref().unwrap_or(""))
        .await
}

/// List live assets, optionally filtered by kind.
///
/// `kind` must be one of the canonical strings or absent / null for "all".
/// Frontend calls this as `"asset_list_lib"` to distinguish it from the base
/// `"asset_list"` command (which uses the free-text kind column).
#[tauri::command]
pub async fn asset_list_lib(
    state: State<'_, AppState>,
    kind: Option<String>,
) -> AppResult<Vec<AssetLibEntry>> {
    let filter = kind.as_deref().map(parse_kind).transpose()?;
    AssetLibRepo::new(state.db.clone()).list(filter).await
}

/// Soft-delete an asset from the library.
/// Called as `"asset_delete_lib"` to avoid shadowing the base `asset_delete`.
#[tauri::command]
pub async fn asset_delete_lib(state: State<'_, AppState>, id: String) -> AppResult<()> {
    AssetLibRepo::new(state.db.clone()).delete(&id).await
}

/// Open the asset's backing file in the system's default application.
///
/// Resolves the file path via the database, then hands it to the OS via
/// `tauri_plugin_opener`. Returns a `not_found` error if the asset id is
/// unknown, or an `io` error if the file no longer exists on disk.
#[tauri::command]
pub async fn asset_open(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let path = AssetLibRepo::new(state.db.clone())
        .path_for_open(&id)
        .await?;
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_kind(s: &str) -> crate::error::AppResult<AssetKind> {
    match s {
        "Logo" => Ok(AssetKind::Logo),
        "Template" => Ok(AssetKind::Template),
        "SongSheet" => Ok(AssetKind::SongSheet),
        "RecurringBlock" => Ok(AssetKind::RecurringBlock),
        "Font" => Ok(AssetKind::Font),
        other => Err(crate::error::AppError::Validation(format!(
            "unknown asset kind '{other}' (expected Logo | Template | SongSheet | RecurringBlock | Font)"
        ))),
    }
}
