//! Typed document template system (Phase doc-templates).
//!
//! `DocTemplate` is a richer template type than the original bare `Template`:
//! it carries a `kind` enum (`Bulletin`, `SongSheet`, …), a validated Typst
//! source string, typed variable definitions, and an optional PNG preview blob.
//!
//! The `template_render` command does a lightweight variable-substitution into
//! the Typst source (replacing `{{VAR_NAME}}` placeholders). Producing an
//! actual PDF via the Typst compiler stays behind the `typst` cargo feature
//! (infra); this layer returns the substituted source string instead — callers
//! that want PDF bytes must compile it separately.
//!
//! Three built-in templates are seeded by `DocTemplateRepo::seed_builtins` on
//! first use. Calling it on an already-seeded database is a no-op.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::db::{now_ms, Db};

// ── Kind enum ────────────────────────────────────────────────────────────────

/// The high-level purpose of a document template.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/DocTemplateKind.ts")]
pub enum DocTemplateKind {
    Bulletin,
    SongSheet,
    Magazine,
    Poster,
    Form,
    LargeText,
}

impl DocTemplateKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocTemplateKind::Bulletin => "Bulletin",
            DocTemplateKind::SongSheet => "SongSheet",
            DocTemplateKind::Magazine => "Magazine",
            DocTemplateKind::Poster => "Poster",
            DocTemplateKind::Form => "Form",
            DocTemplateKind::LargeText => "LargeText",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "Bulletin" => Ok(DocTemplateKind::Bulletin),
            "SongSheet" => Ok(DocTemplateKind::SongSheet),
            "Magazine" => Ok(DocTemplateKind::Magazine),
            "Poster" => Ok(DocTemplateKind::Poster),
            "Form" => Ok(DocTemplateKind::Form),
            "LargeText" => Ok(DocTemplateKind::LargeText),
            other => Err(AppError::Validation(format!(
                "unknown template kind '{other}'"
            ))),
        }
    }
}

// ── TemplateVar kind enum ─────────────────────────────────────────────────────

/// Data type of a template variable.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/TemplateVarKind.ts")]
pub enum TemplateVarKind {
    Text,
    Number,
    Date,
    Boolean,
    SongList,
    ScriptureRef,
}

impl TemplateVarKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TemplateVarKind::Text => "Text",
            TemplateVarKind::Number => "Number",
            TemplateVarKind::Date => "Date",
            TemplateVarKind::Boolean => "Boolean",
            TemplateVarKind::SongList => "SongList",
            TemplateVarKind::ScriptureRef => "ScriptureRef",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "Text" => Ok(TemplateVarKind::Text),
            "Number" => Ok(TemplateVarKind::Number),
            "Date" => Ok(TemplateVarKind::Date),
            "Boolean" => Ok(TemplateVarKind::Boolean),
            "SongList" => Ok(TemplateVarKind::SongList),
            "ScriptureRef" => Ok(TemplateVarKind::ScriptureRef),
            other => Err(AppError::Validation(format!(
                "unknown variable kind '{other}'"
            ))),
        }
    }
}

// ── TemplateVar ───────────────────────────────────────────────────────────────

/// A typed variable declaration inside a document template.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/TemplateVar.ts")]
pub struct TemplateVar {
    pub id: String,
    pub template_id: String,
    /// Key used as `{{name}}` placeholder in the Typst source.
    pub name: String,
    /// Human-readable label shown in the fill-in UI.
    pub label: String,
    /// What type of value this variable accepts.
    pub kind: String,
    pub default_value: Option<String>,
    /// Whether a value must be provided before rendering.
    pub required: bool,
    pub position: i64,
    pub created_at: i64,
}

// ── DocTemplate ───────────────────────────────────────────────────────────────

/// A typed document template with Typst source and variable spec.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/DocTemplate.ts")]
pub struct DocTemplate {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub typst_source: String,
    /// PNG thumbnail — `None` until explicitly generated / stored.
    pub preview_png: Option<Vec<u8>>,
    pub variables: Vec<TemplateVar>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

// ── Input structs ─────────────────────────────────────────────────────────────

/// Input for creating a template variable.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/TemplateVarInput.ts")]
pub struct TemplateVarInput {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub default_value: Option<String>,
    pub required: bool,
}

// ── Repo ─────────────────────────────────────────────────────────────────────

pub struct DocTemplateRepo {
    pub db: Db,
}

/// SQLite row shape — no `variables` column; loaded separately.
#[derive(sqlx::FromRow)]
struct DocTemplateRow {
    id: String,
    name: String,
    kind: String,
    typst_source: String,
    preview_png: Option<Vec<u8>>,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

/// SQLite row shape for template_var.
#[derive(sqlx::FromRow)]
struct TemplateVarRow {
    id: String,
    template_id: String,
    name: String,
    label: String,
    kind: String,
    default_value: Option<String>,
    required: i64,
    position: i64,
    created_at: i64,
}

impl DocTemplateRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    // ── Create ────────────────────────────────────────────────────────────────

    /// Create a new document template with its variable spec.
    pub async fn create(
        &self,
        name: &str,
        kind: &str,
        typst_source: &str,
        variables: &[TemplateVarInput],
    ) -> AppResult<DocTemplate> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("template name is required".into()));
        }
        // Validate kind.
        DocTemplateKind::parse(kind)?;
        // Validate ALL variable kinds up front, before any INSERT, so a bad var
        // at any index can never leave a half-built template behind.
        for var in variables {
            TemplateVarKind::parse(&var.kind)?;
        }

        let id = Uuid::now_v7().to_string();
        let now = now_ms();

        // Parent row + variables are one atomic unit: a failure anywhere rolls
        // the whole thing back (same posture as BlockRepo::reorder).
        let mut tx = self.db.pool.begin().await?;
        sqlx::query(
            "INSERT INTO doc_template (id, name, kind, typst_source, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(kind)
        .bind(typst_source)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Insert variable definitions.
        for (pos, var) in variables.iter().enumerate() {
            let var_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO template_var \
                     (id, template_id, name, label, kind, default_value, required, position, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&var_id)
            .bind(&id)
            .bind(&var.name)
            .bind(&var.label)
            .bind(&var.kind)
            .bind(&var.default_value)
            .bind(if var.required { 1i64 } else { 0i64 })
            .bind(pos as i64)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        self.get(&id).await
    }

    // ── Get ───────────────────────────────────────────────────────────────────

    pub async fn get(&self, id: &str) -> AppResult<DocTemplate> {
        let row = sqlx::query_as::<_, DocTemplateRow>(
            "SELECT id, name, kind, typst_source, preview_png, created_at, updated_at, deleted_at \
             FROM doc_template WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.db.pool)
        .await?
        .ok_or_else(|| AppError::NotFound {
            entity: "doc_template",
            id: id.to_string(),
        })?;

        let variables = self.load_vars(id).await?;
        Ok(row_to_template(row, variables))
    }

    // ── List ──────────────────────────────────────────────────────────────────

    /// All live templates, optionally filtered by kind. Alphabetical by name.
    pub async fn list(&self, kind: Option<&str>) -> AppResult<Vec<DocTemplate>> {
        let rows = if let Some(k) = kind {
            sqlx::query_as::<_, DocTemplateRow>(
                "SELECT id, name, kind, typst_source, preview_png, created_at, updated_at, deleted_at \
                 FROM doc_template WHERE deleted_at IS NULL AND kind = ? \
                 ORDER BY name COLLATE NOCASE ASC",
            )
            .bind(k)
            .fetch_all(&self.db.pool)
            .await?
        } else {
            sqlx::query_as::<_, DocTemplateRow>(
                "SELECT id, name, kind, typst_source, preview_png, created_at, updated_at, deleted_at \
                 FROM doc_template WHERE deleted_at IS NULL \
                 ORDER BY name COLLATE NOCASE ASC",
            )
            .fetch_all(&self.db.pool)
            .await?
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let vars = self.load_vars(&row.id).await?;
            out.push(row_to_template(row, vars));
        }
        Ok(out)
    }

    // ── Update ────────────────────────────────────────────────────────────────

    /// Update the header fields of a template (does not touch variables).
    pub async fn update(
        &self,
        id: &str,
        name: &str,
        kind: &str,
        typst_source: &str,
    ) -> AppResult<DocTemplate> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("template name is required".into()));
        }
        DocTemplateKind::parse(kind)?;

        let affected = sqlx::query(
            "UPDATE doc_template \
             SET name = ?, kind = ?, typst_source = ?, updated_at = ? \
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(kind)
        .bind(typst_source)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();

        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "doc_template",
                id: id.to_string(),
            });
        }
        self.get(id).await
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let affected = sqlx::query(
            "UPDATE doc_template SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();

        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "doc_template",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // ── Render (variable substitution) ────────────────────────────────────────

    /// Substitute `{{VAR_NAME}}` placeholders in the Typst source with the
    /// supplied values. Returns the rendered source string (not compiled PDF).
    /// Missing required variables cause a `Validation` error. Unknown variables
    /// in `vars` are silently ignored (forward-compat).
    pub async fn render(
        &self,
        id: &str,
        vars: &HashMap<String, String>,
    ) -> AppResult<String> {
        let tmpl = self.get(id).await?;

        // Check required variables are supplied.
        for var in &tmpl.variables {
            if var.required && !vars.contains_key(&var.name) && var.default_value.is_none() {
                return Err(AppError::Validation(format!(
                    "required variable '{}' not provided",
                    var.name
                )));
            }
        }

        // Perform substitution: {{VAR_NAME}} → value (or default, or empty).
        let mut source = tmpl.typst_source.clone();
        for var in &tmpl.variables {
            let placeholder = format!("{{{{{}}}}}", var.name);
            let value = vars
                .get(&var.name)
                .map(String::as_str)
                .or(var.default_value.as_deref())
                .unwrap_or("");
            source = source.replace(&placeholder, value);
        }
        Ok(source)
    }

    // ── Preview PNG ───────────────────────────────────────────────────────────

    /// Store (or replace) the PNG preview for a template.
    pub async fn set_preview_png(&self, id: &str, png: &[u8]) -> AppResult<()> {
        let affected = sqlx::query(
            "UPDATE doc_template SET preview_png = ?, updated_at = ? \
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(png)
        .bind(now_ms())
        .bind(id)
        .execute(&self.db.pool)
        .await?
        .rows_affected();

        if affected == 0 {
            return Err(AppError::NotFound {
                entity: "doc_template",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // ── Seed builtins ─────────────────────────────────────────────────────────

    /// Insert the three built-in templates if they have not been seeded yet.
    /// Idempotent: skips any template whose name already exists.
    pub async fn seed_builtins(&self) -> AppResult<()> {
        for (name, kind, source, vars) in builtin_templates() {
            let exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM doc_template WHERE name = ? AND deleted_at IS NULL LIMIT 1",
            )
            .bind(name)
            .fetch_optional(&self.db.pool)
            .await?;

            if exists.is_none() {
                self.create(name, kind, source, &vars).await?;
            }
        }
        Ok(())
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    async fn load_vars(&self, template_id: &str) -> AppResult<Vec<TemplateVar>> {
        let rows = sqlx::query_as::<_, TemplateVarRow>(
            "SELECT id, template_id, name, label, kind, default_value, required, position, created_at \
             FROM template_var WHERE template_id = ? ORDER BY position ASC, id ASC",
        )
        .bind(template_id)
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_var).collect())
    }
}

// ── Row mappers ───────────────────────────────────────────────────────────────

fn row_to_template(row: DocTemplateRow, variables: Vec<TemplateVar>) -> DocTemplate {
    DocTemplate {
        id: row.id,
        name: row.name,
        kind: row.kind,
        typst_source: row.typst_source,
        preview_png: row.preview_png,
        variables,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    }
}

fn row_to_var(row: TemplateVarRow) -> TemplateVar {
    TemplateVar {
        id: row.id,
        template_id: row.template_id,
        name: row.name,
        label: row.label,
        kind: row.kind,
        default_value: row.default_value,
        required: row.required != 0,
        position: row.position,
        created_at: row.created_at,
    }
}

// ── Built-in template seed data ───────────────────────────────────────────────

/// Returns (name, kind_str, typst_source, variables) for each builtin template.
fn builtin_templates() -> Vec<(&'static str, &'static str, &'static str, Vec<TemplateVarInput>)> {
    vec![
        (
            "Gudstjeneste-program (standard)",
            "Bulletin",
            BULLETIN_STANDARD,
            vec![
                TemplateVarInput {
                    name: "church_name".into(),
                    label: "Menighetsnavn".into(),
                    kind: "Text".into(),
                    default_value: Some("Vår menighet".into()),
                    required: false,
                },
                TemplateVarInput {
                    name: "service_title".into(),
                    label: "Tjenestens tittel".into(),
                    kind: "Text".into(),
                    default_value: Some("Gudstjeneste".into()),
                    required: true,
                },
                TemplateVarInput {
                    name: "date".into(),
                    label: "Dato".into(),
                    kind: "Date".into(),
                    default_value: None,
                    required: true,
                },
                TemplateVarInput {
                    name: "preacher".into(),
                    label: "Forkynner".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "songs".into(),
                    label: "Sang-liste".into(),
                    kind: "SongList".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "announcements".into(),
                    label: "Kunngjøringer".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
            ],
        ),
        (
            "Sangark A4 (to kolonner)",
            "SongSheet",
            SONGSHEET_A4,
            vec![
                TemplateVarInput {
                    name: "song_title".into(),
                    label: "Sangtittel".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: true,
                },
                TemplateVarInput {
                    name: "song_number".into(),
                    label: "Sangnummer".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "author".into(),
                    label: "Forfatter".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "lyrics".into(),
                    label: "Sangtekst".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: true,
                },
                TemplateVarInput {
                    name: "copyright".into(),
                    label: "Opphavsrett".into(),
                    kind: "Text".into(),
                    default_value: Some("Alle rettigheter forbeholdt".into()),
                    required: false,
                },
            ],
        ),
        (
            "Kunngjøringsark A5",
            "Poster",
            ANNOUNCEMENT_A5,
            vec![
                TemplateVarInput {
                    name: "title".into(),
                    label: "Tittel".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: true,
                },
                TemplateVarInput {
                    name: "body".into(),
                    label: "Innhold".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: true,
                },
                TemplateVarInput {
                    name: "date_time".into(),
                    label: "Dato og tid".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "location".into(),
                    label: "Sted".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
                TemplateVarInput {
                    name: "contact".into(),
                    label: "Kontakt".into(),
                    kind: "Text".into(),
                    default_value: None,
                    required: false,
                },
            ],
        ),
    ]
}

// ── Built-in Typst source strings ─────────────────────────────────────────────

/// Standard bulletin (gudstjeneste-program) — header, liturgy, song list,
/// announcements section.
const BULLETIN_STANDARD: &str = r#"// Generated by SundayPaper — bulletin_standard
// Variables: {{church_name}}, {{service_title}}, {{date}}, {{preacher}},
//            {{songs}}, {{announcements}}

#set page(paper: "a4", margin: (x: 2.5cm, y: 2cm))
#set text(size: 11pt, font: "Linux Libertine")
#set par(justify: false, leading: 0.65em)

// Header
#align(center)[
  #text(size: 0.9em, fill: gray)[{{church_name}}]
  #v(0.3em)
  #text(size: 1.8em, weight: "bold")[{{service_title}}]
  #v(0.2em)
  #text(size: 1.1em, style: "italic")[{{date}}]
]
#v(0.5em)
#line(length: 100%, stroke: 0.5pt + gray)
#v(0.8em)

// Liturgy section
#grid(columns: (auto, 1fr), gutter: 0.5em,
  [*Forkynner:*], [{{preacher}}],
)
#v(1em)

// Song list
#if "{{songs}}" != "" [
  == Sanger
  {{songs}}
  #v(0.5em)
]

// Announcements
#if "{{announcements}}" != "" [
  == Kunngjøringer
  #par[{{announcements}}]
]

#v(1fr)
#align(center)[
  #text(size: 0.75em, fill: gray)[Laget med SundayPaper]
]
"#;

/// A4 two-column song sheet for printing.
const SONGSHEET_A4: &str = r#"// Generated by SundayPaper — songsheet_a4
// Variables: {{song_title}}, {{song_number}}, {{author}}, {{lyrics}}, {{copyright}}

#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(size: 11pt)
#set par(leading: 0.7em)

// Title block
#align(center)[
  #if "{{song_number}}" != "" [
    #text(size: 0.9em, fill: gray)[Nr. {{song_number}}]
    #v(0.2em)
  ]
  #text(size: 1.6em, weight: "bold")[{{song_title}}]
  #v(0.2em)
  #if "{{author}}" != "" [
    #text(size: 0.9em, style: "italic")[{{author}}]
  ]
]
#v(0.6em)
#line(length: 100%, stroke: 0.5pt + gray)
#v(0.8em)

// Lyrics in two columns
#columns(2, gutter: 1.5em)[
  {{lyrics}}
]

#v(1fr)
#align(center)[
  #text(size: 0.75em, fill: gray)[{{copyright}}]
]
"#;

/// A5 announcement sheet with title, body, and optional details.
const ANNOUNCEMENT_A5: &str = r##"// Generated by SundayPaper — announcement_a5
// Variables: {{title}}, {{body}}, {{date_time}}, {{location}}, {{contact}}

#set page(paper: "a5", margin: (x: 1.8cm, y: 1.8cm))
#set text(size: 11pt)
#set par(justify: true, leading: 0.7em)

// Top accent bar
#block(
  width: 100%,
  height: 0.4cm,
  fill: rgb("#1a3a5c"),
)
#v(0.6em)

// Title
#align(center)[
  #text(size: 1.5em, weight: "bold")[{{title}}]
]
#v(0.4em)
#line(length: 100%, stroke: 0.4pt + luma(180))
#v(0.6em)

// Body text
#par[{{body}}]

// Detail rows (only shown when non-empty)
#if "{{date_time}}" != "" or "{{location}}" != "" [
  #v(0.8em)
  #set text(size: 0.9em)
  #grid(columns: (auto, 1fr), gutter: (0.4em, 0.3em),
    [*Tid:*],    [{{date_time}}],
    [*Sted:*],   [{{location}}],
  )
]

#if "{{contact}}" != "" [
  #v(0.5em)
  #text(size: 0.85em, style: "italic")[Kontakt: {{contact}}]
]

#v(1fr)
#align(center)[
  #text(size: 0.7em, fill: gray)[SundayPaper]
]
"##;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> DocTemplateRepo {
        DocTemplateRepo::new(Db::connect_memory().await.expect("connect"))
    }

    // ── Kind / var kind round-trips ───────────────────────────────────────────

    #[test]
    fn doc_template_kind_round_trip() {
        for kind in [
            DocTemplateKind::Bulletin,
            DocTemplateKind::SongSheet,
            DocTemplateKind::Magazine,
            DocTemplateKind::Poster,
            DocTemplateKind::Form,
            DocTemplateKind::LargeText,
        ] {
            let s = kind.as_str();
            let back = DocTemplateKind::parse(s).unwrap();
            assert_eq!(kind, back, "round-trip failed for {s}");
        }
    }

    #[test]
    fn doc_template_kind_rejects_unknown() {
        assert!(matches!(
            DocTemplateKind::parse("Flyer").unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[test]
    fn template_var_kind_round_trip() {
        for kind in [
            TemplateVarKind::Text,
            TemplateVarKind::Number,
            TemplateVarKind::Date,
            TemplateVarKind::Boolean,
            TemplateVarKind::SongList,
            TemplateVarKind::ScriptureRef,
        ] {
            let s = kind.as_str();
            let back = TemplateVarKind::parse(s).unwrap();
            assert_eq!(kind, back, "round-trip failed for {s}");
        }
    }

    #[test]
    fn template_var_kind_rejects_unknown() {
        assert!(matches!(
            TemplateVarKind::parse("Image").unwrap_err(),
            AppError::Validation(_)
        ));
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_roundtrip() {
        let repo = repo().await;
        let vars = vec![TemplateVarInput {
            name: "title".into(),
            label: "Tittel".into(),
            kind: "Text".into(),
            default_value: Some("Søndagsgudstjeneste".into()),
            required: true,
        }];
        let tmpl = repo
            .create("Bulletin test", "Bulletin", "// source", &vars)
            .await
            .unwrap();
        assert_eq!(tmpl.name, "Bulletin test");
        assert_eq!(tmpl.kind, "Bulletin");
        assert_eq!(tmpl.variables.len(), 1);
        assert_eq!(tmpl.variables[0].name, "title");
        assert!(tmpl.variables[0].required);

        // get by id returns same data
        let fetched = repo.get(&tmpl.id).await.unwrap();
        assert_eq!(fetched.id, tmpl.id);
        assert_eq!(fetched.variables.len(), 1);
    }

    #[tokio::test]
    async fn create_validates_name() {
        let repo = repo().await;
        assert!(matches!(
            repo.create("  ", "Bulletin", "", &[]).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn create_validates_kind() {
        let repo = repo().await;
        assert!(matches!(
            repo.create("X", "Unknown", "", &[]).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn create_validates_var_kind() {
        let repo = repo().await;
        let bad_var = vec![TemplateVarInput {
            name: "x".into(),
            label: "X".into(),
            kind: "BadType".into(),
            default_value: None,
            required: false,
        }];
        assert!(matches!(
            repo.create("T", "Bulletin", "", &bad_var)
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));
    }

    /// A bad var at index >= 1 must leave NOTHING persisted: create() is atomic.
    /// The naive (non-transactional, validate-mid-loop) implementation INSERTs
    /// the parent row + var 0 before hitting the bad var 1, so an orphaned
    /// template "T" plus var "a" would linger after the Err.
    #[tokio::test]
    async fn create_with_bad_var_at_index_one_persists_nothing() {
        let repo = repo().await;
        let vars = vec![
            TemplateVarInput {
                name: "a".into(),
                label: "A".into(),
                kind: "Text".into(),
                default_value: None,
                required: false,
            },
            TemplateVarInput {
                name: "b".into(),
                label: "B".into(),
                kind: "BadType".into(),
                default_value: None,
                required: false,
            },
        ];
        assert!(matches!(
            repo.create("T", "Bulletin", "src", &vars)
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));

        // No ghost template, and no orphaned variable row.
        assert!(
            repo.list(None).await.unwrap().is_empty(),
            "no orphaned template should remain after a failed create"
        );
        let var_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM template_var")
            .fetch_one(&repo.db.pool)
            .await
            .unwrap();
        assert_eq!(var_count, 0, "no orphaned template_var rows should remain");
    }

    #[tokio::test]
    async fn list_returns_all_live_sorted() {
        let repo = repo().await;
        repo.create("Zzz template", "Form", "", &[]).await.unwrap();
        repo.create("Aaa template", "Poster", "", &[]).await.unwrap();
        let list = repo.list(None).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Aaa template");
        assert_eq!(list[1].name, "Zzz template");
    }

    #[tokio::test]
    async fn list_filter_by_kind() {
        let repo = repo().await;
        repo.create("B1", "Bulletin", "", &[]).await.unwrap();
        repo.create("B2", "Bulletin", "", &[]).await.unwrap();
        repo.create("S1", "SongSheet", "", &[]).await.unwrap();

        let bulletins = repo.list(Some("Bulletin")).await.unwrap();
        assert_eq!(bulletins.len(), 2);
        let songsheets = repo.list(Some("SongSheet")).await.unwrap();
        assert_eq!(songsheets.len(), 1);
    }

    #[tokio::test]
    async fn update_changes_fields() {
        let repo = repo().await;
        let tmpl = repo.create("Old name", "Form", "// old", &[]).await.unwrap();
        let updated = repo
            .update(&tmpl.id, "New name", "Poster", "// new source")
            .await
            .unwrap();
        assert_eq!(updated.name, "New name");
        assert_eq!(updated.kind, "Poster");
        assert_eq!(updated.typst_source, "// new source");
    }

    #[tokio::test]
    async fn update_not_found() {
        let repo = repo().await;
        assert!(matches!(
            repo.update("no-such-id", "X", "Bulletin", "")
                .await
                .unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn delete_soft_deletes() {
        let repo = repo().await;
        let tmpl = repo
            .create("To delete", "Magazine", "", &[])
            .await
            .unwrap();
        repo.delete(&tmpl.id).await.unwrap();
        assert!(repo.list(None).await.unwrap().is_empty());
        assert!(matches!(
            repo.get(&tmpl.id).await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn delete_not_found() {
        let repo = repo().await;
        assert!(matches!(
            repo.delete("ghost-id").await.unwrap_err(),
            AppError::NotFound { .. }
        ));
    }

    // ── Render ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn render_substitutes_variables() {
        let repo = repo().await;
        let vars = vec![
            TemplateVarInput {
                name: "title".into(),
                label: "Tittel".into(),
                kind: "Text".into(),
                default_value: None,
                required: true,
            },
            TemplateVarInput {
                name: "date".into(),
                label: "Dato".into(),
                kind: "Date".into(),
                default_value: Some("2026-06-01".into()),
                required: false,
            },
        ];
        let tmpl = repo
            .create(
                "Test render",
                "Bulletin",
                "Service: {{title}} on {{date}}",
                &vars,
            )
            .await
            .unwrap();

        let mut provided = HashMap::new();
        provided.insert("title".to_string(), "Søndagsgudstjeneste".to_string());

        let rendered = repo.render(&tmpl.id, &provided).await.unwrap();
        assert_eq!(
            rendered,
            "Service: Søndagsgudstjeneste on 2026-06-01",
            "title replaced; date falls back to default"
        );
    }

    #[tokio::test]
    async fn render_uses_caller_value_over_default() {
        let repo = repo().await;
        let vars = vec![TemplateVarInput {
            name: "title".into(),
            label: "T".into(),
            kind: "Text".into(),
            default_value: Some("Default".into()),
            required: false,
        }];
        let tmpl = repo
            .create("Render pref", "Bulletin", "Hello {{title}}", &vars)
            .await
            .unwrap();

        let mut provided = HashMap::new();
        provided.insert("title".to_string(), "Overridden".to_string());

        let rendered = repo.render(&tmpl.id, &provided).await.unwrap();
        assert_eq!(rendered, "Hello Overridden");
    }

    #[tokio::test]
    async fn render_missing_required_variable_is_error() {
        let repo = repo().await;
        let vars = vec![TemplateVarInput {
            name: "must_have".into(),
            label: "Must".into(),
            kind: "Text".into(),
            default_value: None,
            required: true,
        }];
        let tmpl = repo
            .create("Strict", "Form", "{{must_have}}", &vars)
            .await
            .unwrap();
        let result = repo.render(&tmpl.id, &HashMap::new()).await;
        assert!(matches!(result.unwrap_err(), AppError::Validation(_)));
    }

    #[tokio::test]
    async fn render_unknown_vars_in_map_are_ignored() {
        let repo = repo().await;
        let tmpl = repo
            .create("No vars", "Poster", "static text", &[])
            .await
            .unwrap();
        let mut map = HashMap::new();
        map.insert("ghost".to_string(), "ignored".to_string());
        let rendered = repo.render(&tmpl.id, &map).await.unwrap();
        assert_eq!(rendered, "static text");
    }

    // ── Preview PNG ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn preview_png_stored_and_retrieved() {
        let repo = repo().await;
        let tmpl = repo
            .create("PNG test", "Bulletin", "", &[])
            .await
            .unwrap();
        assert!(tmpl.preview_png.is_none(), "no preview initially");
        let fake_png = vec![0x89u8, 0x50, 0x4e, 0x47]; // PNG magic bytes
        repo.set_preview_png(&tmpl.id, &fake_png).await.unwrap();
        let fetched = repo.get(&tmpl.id).await.unwrap();
        assert_eq!(fetched.preview_png.as_deref(), Some(fake_png.as_slice()));
    }

    // ── Seed builtins ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn seed_builtins_creates_three_templates() {
        let repo = repo().await;
        repo.seed_builtins().await.unwrap();
        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 3, "three built-in templates");
    }

    #[tokio::test]
    async fn seed_builtins_is_idempotent() {
        let repo = repo().await;
        repo.seed_builtins().await.unwrap();
        repo.seed_builtins().await.unwrap(); // second call must be safe
        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 3, "still only three after double seed");
    }

    #[tokio::test]
    async fn seed_builtins_have_correct_kinds() {
        let repo = repo().await;
        repo.seed_builtins().await.unwrap();
        let bulletins = repo.list(Some("Bulletin")).await.unwrap();
        assert_eq!(bulletins.len(), 1);
        let songsheets = repo.list(Some("SongSheet")).await.unwrap();
        assert_eq!(songsheets.len(), 1);
        // Kunngjøringsark A5 is Poster kind
        let posters = repo.list(Some("Poster")).await.unwrap();
        assert_eq!(posters.len(), 1);
    }

    #[tokio::test]
    async fn seed_bulletin_has_variables() {
        let repo = repo().await;
        repo.seed_builtins().await.unwrap();
        let bulletins = repo.list(Some("Bulletin")).await.unwrap();
        let b = &bulletins[0];
        assert!(b.variables.len() >= 3, "bulletin has at least 3 vars");
        let required_names: Vec<_> = b.variables.iter().filter(|v| v.required).collect();
        assert!(!required_names.is_empty(), "some vars are required");
    }

    #[tokio::test]
    async fn builtin_bulletin_source_contains_placeholders() {
        let repo = repo().await;
        repo.seed_builtins().await.unwrap();
        let bulletins = repo.list(Some("Bulletin")).await.unwrap();
        let b = &bulletins[0];
        assert!(
            b.typst_source.contains("{{service_title}}"),
            "bulletin source has {{{{service_title}}}} placeholder"
        );
        assert!(
            b.typst_source.contains("{{date}}"),
            "bulletin source has {{{{date}}}} placeholder"
        );
    }
}
