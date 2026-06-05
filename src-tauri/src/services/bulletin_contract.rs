//! The **canonical** SundayPlan → Paper bridge: adapt the *published* shared
//! `sunday-contracts` `ServicePlan` onto Paper's local plan model, then reuse
//! the already-tested [`build_bulletin`] block generator.
//!
//! ## Why this module exists
//!
//! `bulletin.rs` defines a *local mirror* of the plan contract (`ServicePlan`,
//! `SetlistItem`, `SongRef`, …) and the pure block generator [`build_bulletin`].
//! That mirror was written before the shared platform contract was published and
//! it **diverged** from it: the published contract (see
//! `sunday-platform/packages/contracts/src/service.ts` and its Rust twin
//! `crates/sunday-contracts`) wraps the items in a `service` envelope, keys song
//! references on `sundaysong_id` / `ccli_song_id`, carries `scripture_ref` as a
//! flat `"Book chapter:verse"` string, and uses a *superset* item-kind enum
//! (`reading`, `response`, `media`, `gap`, `custom`, …) that Plan and Stage both
//! map onto.
//!
//! A real plan pulled from SundayPlan therefore arrives in the **canonical**
//! shape — exactly what the golden fixture
//! `sunday-platform/fixtures/service_plan.json` captures — and would *not*
//! deserialise into the local mirror. This module closes that gap with a single
//! pure function, [`bulletin_from_contract`], that:
//!
//!   1. accepts the canonical [`ContractServicePlan`] (a faithful mirror of the
//!      published contract, so the golden fixture deserialises verbatim), then
//!   2. lowers it onto the local [`bulletin::ServicePlan`], then
//!   3. delegates to the existing, exhaustively-tested [`build_bulletin`].
//!
//! No block-shaping logic is duplicated: the canonical item kinds are mapped
//! onto the local kinds and all of `build_bulletin`'s per-kind payload rules,
//! header handling, hint merging and blank-normalisation are reused as-is.
//!
//! It is **pure** (no I/O, no database, no feature gates) and proven by a
//! golden-fixture round-trip test, same posture as `bulletin` and `pdf::plan`.

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::services::bulletin::{
    build_bulletin, BlockSpec, ScriptureRef, ServicePlan, SetlistItem, SetlistItemKind, SongRef,
};

/// Canonical running-order item kind from the published `sunday-contracts`
/// `ServiceItemKind` — a *superset* of every app's local kinds. Unknown / future
/// values deserialise to [`ContractItemKind::Custom`] via `#[serde(other)]`, so a
/// newer Plan can add kinds without breaking this adapter.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContractItemKind {
    Song,
    Scripture,
    Sermon,
    Reading,
    Prayer,
    Offering,
    Announcement,
    Welcome,
    Response,
    Media,
    Gap,
    #[serde(other)]
    #[default]
    Custom,
}

/// Canonical song reference from the published contract's `SongRef`. We only
/// pull through the fields Paper's block model can carry today; the rest
/// (`default_key`, `language`, `local_id` …) are accepted so the fixture
/// deserialises but are intentionally not yet rendered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContractSongRef {
    #[serde(default)]
    pub sundaysong_id: Option<String>,
    #[serde(default)]
    pub local_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub ccli_song_id: Option<String>,
    #[serde(default)]
    pub tono_work_id: Option<String>,
    #[serde(default)]
    pub default_key: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

/// One canonical running-order row (`sunday-contracts` `SetlistItem`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContractSetlistItem {
    #[serde(default)]
    pub position: i64,
    pub kind: ContractItemKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub song_ref: Option<ContractSongRef>,
    /// A flat `"Book chapter:verse"` reference, e.g. `"John 3:16-21"`.
    #[serde(default)]
    pub scripture_ref: Option<String>,
    #[serde(default)]
    pub key_override: Option<String>,
    #[serde(default)]
    pub duration_min: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// The `service` envelope (`sunday-contracts` `ServiceRef`). Only the header
/// fields Paper prints are pulled through; the rest deserialise but are unused.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContractServiceRef {
    #[serde(default)]
    pub name: Option<String>,
    /// ISO 8601 UTC timestamp, e.g. `"2026-05-31T09:00:00Z"`. Carried verbatim
    /// as the printed date — Paper does not reformat (the plan owns the locale).
    #[serde(default)]
    pub starts_at: Option<String>,
}

/// The published-contract `ServicePlan`: a `service` envelope plus its ordered
/// `items`. Mirrors `sunday-contracts` faithfully enough that the golden fixture
/// `service_plan.json` deserialises verbatim; extra contract fields
/// (`schema_version`, `church_id`, `state`, …) are simply ignored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContractServicePlan {
    #[serde(default)]
    pub service: ContractServiceRef,
    #[serde(default)]
    pub items: Vec<ContractSetlistItem>,
}

/// Map a published-contract item kind onto Paper's local block-shaping kind.
///
/// The local kinds Paper renders are richer in some places and coarser in
/// others, so the mapping is deliberate:
///
/// - `song` / `scripture` / `sermon` / `prayer` / `offering` / `announcement`
///   / `welcome` map one-to-one.
/// - `reading` → `scripture` (a reading prints with the scripture block; if it
///   carries no reference the block simply shows the title + text).
/// - `response` → `liturgy` (a sung/spoken congregational response).
/// - `media` → `image` (a slide / video / poster the planner attached).
/// - `gap` / `custom` / any unknown future kind → `other`, which
///   [`build_bulletin`] lowers to a safe `text` block so nothing is dropped.
fn local_kind(kind: ContractItemKind) -> SetlistItemKind {
    match kind {
        ContractItemKind::Song => SetlistItemKind::Song,
        ContractItemKind::Scripture | ContractItemKind::Reading => SetlistItemKind::Scripture,
        ContractItemKind::Sermon => SetlistItemKind::Sermon,
        ContractItemKind::Prayer => SetlistItemKind::Prayer,
        ContractItemKind::Offering => SetlistItemKind::Offering,
        ContractItemKind::Announcement => SetlistItemKind::Announcement,
        ContractItemKind::Welcome => SetlistItemKind::Welcome,
        ContractItemKind::Response => SetlistItemKind::Liturgy,
        ContractItemKind::Media => SetlistItemKind::Image,
        ContractItemKind::Gap | ContractItemKind::Custom => SetlistItemKind::Other,
    }
}

/// Split a flat scripture reference (`"John 3:16-21"`, `"1 Corinthians 13"`)
/// into `(book, reference)`. The book is everything up to the last
/// whitespace-separated token *iff* that token contains a digit (the chapter /
/// verse part); otherwise the whole string is treated as the book (e.g. a named
/// pericope with no numbers). Returns the trimmed pieces, dropping blanks.
fn split_scripture(raw: &str) -> (Option<String>, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    match trimmed.rsplit_once(char::is_whitespace) {
        Some((book, reference)) if reference.chars().any(|c| c.is_ascii_digit()) => {
            (non_blank(book), non_blank(reference))
        }
        // No trailing numeric reference token → it's all "book".
        _ => (non_blank(trimmed), None),
    }
}

/// Trim and drop-if-blank, returning an owned `Option<String>`.
fn non_blank(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Lower one canonical item onto the local [`SetlistItem`] the block generator
/// understands. Carries the title, the song / scripture references, and the
/// planner `notes` (as the item body so it prints). The canonical
/// `key_override` and `duration_min` run-sheet fields have no home in Paper's
/// current block model and are intentionally dropped here (see module notes).
fn lower_item(item: ContractSetlistItem) -> SetlistItem {
    let kind = local_kind(item.kind);

    let song = item.song_ref.map(|s| SongRef {
        // Prefer the suite-wide id; fall back to the plan's local id.
        song_id: s.sundaysong_id.or(s.local_id),
        tono_work_id: s.tono_work_id,
        author: None,
        // CCLI number is the printable catalogue number we have.
        number: s.ccli_song_id,
        verses: Vec::new(),
        refrain: None,
    });

    let scripture = item
        .scripture_ref
        .as_deref()
        .map(split_scripture)
        .map(|(book, reference)| ScriptureRef {
            book,
            reference,
            translation: None,
        });

    SetlistItem {
        kind,
        title: item.title,
        // The planner's free-text notes become the printed body.
        body: item.notes,
        leader: None,
        time: None,
        copyright: None,
        page_break: false,
        // Only attach refs for the kinds that own them, mirroring how the local
        // contract shapes a plan, so `build_bulletin`'s `unwrap_or_default`
        // branches behave identically to a hand-built local plan.
        song: if kind == SetlistItemKind::Song {
            song
        } else {
            None
        },
        scripture: if kind == SetlistItemKind::Scripture {
            scripture
        } else {
            None
        },
        asset: None,
    }
}

/// Lower a whole canonical [`ContractServicePlan`] onto the local
/// [`ServicePlan`]. Items are taken in the order they arrive (the contract's
/// `position` field is the source of truth for order, and a well-formed plan is
/// already ordered; we preserve the given order verbatim rather than re-sorting
/// so the program matches the plan exactly).
pub fn lower_contract_plan(plan: ContractServicePlan) -> ServicePlan {
    ServicePlan {
        title: plan.service.name,
        // SundayPlan's `service` envelope has no separate congregation field
        // today; the church is identified by `church_id` only. Leave the printed
        // subtitle empty rather than printing a UUID.
        church: None,
        date: plan.service.starts_at,
        items: plan.items.into_iter().map(lower_item).collect(),
    }
}

/// The canonical SundayPlan → Paper bridge, end to end: adapt a published
/// `sunday-contracts` [`ContractServicePlan`] into the ordered [`BlockSpec`]s
/// Paper persists and renders, reusing the local [`build_bulletin`] generator.
///
/// Pure; errors only when the plan has no items (delegated to `build_bulletin`).
pub fn bulletin_from_contract(plan: ContractServicePlan) -> AppResult<Vec<BlockSpec>> {
    build_bulletin(&lower_contract_plan(plan))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared platform golden fixture, vendored into this repo's test tree
    /// so the round-trip is hermetic. Keep it byte-identical to
    /// `sunday-platform/fixtures/service_plan.json`.
    const GOLDEN: &str = include_str!("../../tests/fixtures/service_plan.json");

    fn data(spec: &BlockSpec) -> serde_json::Value {
        serde_json::from_str(&spec.data).expect("spec data is valid JSON")
    }

    #[test]
    fn golden_fixture_deserialises_into_the_contract_plan() {
        // The published-contract mirror must accept the canonical fixture
        // verbatim, ignoring envelope fields it doesn't print.
        let plan: ContractServicePlan = serde_json::from_str(GOLDEN).unwrap();
        assert_eq!(plan.service.name.as_deref(), Some("Sunday Morning"));
        assert_eq!(
            plan.service.starts_at.as_deref(),
            Some("2026-05-31T09:00:00Z")
        );
        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.items[0].kind, ContractItemKind::Welcome);
        assert_eq!(plan.items[1].kind, ContractItemKind::Song);
        assert_eq!(plan.items[2].kind, ContractItemKind::Scripture);
        // The song reference keys on the suite-wide id + CCLI number.
        let song = plan.items[1].song_ref.as_ref().unwrap();
        assert_eq!(
            song.sundaysong_id.as_deref(),
            Some("22222222-2222-2222-2222-222222222222")
        );
        assert_eq!(song.ccli_song_id.as_deref(), Some("22025"));
        // Scripture arrives as a flat string.
        assert_eq!(plan.items[2].scripture_ref.as_deref(), Some("John 3:16-21"));
    }

    #[test]
    fn golden_fixture_round_trips_into_the_block_tree() {
        let plan: ContractServicePlan = serde_json::from_str(GOLDEN).unwrap();
        let specs = bulletin_from_contract(plan).unwrap();

        // Header block (service name + date) + one block per item, in order.
        assert_eq!(specs.len(), 4, "header + 3 items");

        // [0] header carries the service title and the verbatim date string.
        assert_eq!(specs[0].kind, "heading");
        let head = data(&specs[0]);
        assert_eq!(head["role"], "service-title");
        assert_eq!(head["title"], "Sunday Morning");
        assert!(
            head["subtitle"].is_null(),
            "no congregation field in contract"
        );
        assert_eq!(head["date"], "2026-05-31T09:00:00Z");

        // [1] welcome → liturgy block with the welcome role + title.
        assert_eq!(specs[1].kind, "liturgy");
        let welcome = data(&specs[1]);
        assert_eq!(welcome["role"], "welcome");
        assert_eq!(welcome["title"], "Welcome & notices");

        // [2] song → song block carrying the suite id + CCLI number as `number`.
        assert_eq!(specs[2].kind, "song");
        let song = data(&specs[2]);
        assert_eq!(song["title"], "Amazing Grace");
        assert_eq!(song["songId"], "22222222-2222-2222-2222-222222222222");
        assert_eq!(song["number"], "22025", "CCLI number prints as the number");
        // No verses bound in the plan → empty list, never a crash.
        assert_eq!(song["verses"].as_array().unwrap().len(), 0);
        // The planner note rides through as nothing breaks (songs ignore body).

        // [3] scripture → scripture block with the parsed book + reference.
        assert_eq!(specs[3].kind, "scripture");
        let scr = data(&specs[3]);
        assert_eq!(scr["title"], "Reading");
        assert_eq!(scr["book"], "John");
        assert_eq!(scr["reference"], "3:16-21");
    }

    #[test]
    fn order_is_preserved_from_the_fixture() {
        let plan: ContractServicePlan = serde_json::from_str(GOLDEN).unwrap();
        let specs = bulletin_from_contract(plan).unwrap();
        let kinds: Vec<&str> = specs.iter().map(|s| s.kind.as_str()).collect();
        // header, then the fixture's welcome → song → scripture order.
        assert_eq!(kinds, vec!["heading", "liturgy", "song", "scripture"]);
    }

    #[test]
    fn scripture_string_splits_into_book_and_reference() {
        assert_eq!(
            split_scripture("John 3:16-21"),
            (Some("John".into()), Some("3:16-21".into()))
        );
        // Multi-word book name: split on the LAST whitespace before the numbers.
        assert_eq!(
            split_scripture("1 Corinthians 13"),
            (Some("1 Corinthians".into()), Some("13".into()))
        );
        // No numeric reference → all book.
        assert_eq!(
            split_scripture("The Beatitudes"),
            (Some("The Beatitudes".into()), None)
        );
        assert_eq!(split_scripture("   "), (None, None));
    }

    #[test]
    fn reading_kind_maps_to_scripture_block() {
        let plan = ContractServicePlan {
            items: vec![ContractSetlistItem {
                kind: ContractItemKind::Reading,
                title: Some("First Reading".into()),
                scripture_ref: Some("Romans 8:28".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = bulletin_from_contract(plan).unwrap();
        assert_eq!(specs[0].kind, "scripture");
        let d = data(&specs[0]);
        assert_eq!(d["book"], "Romans");
        assert_eq!(d["reference"], "8:28");
    }

    #[test]
    fn superset_kinds_map_or_fall_back_safely() {
        let plan = ContractServicePlan {
            items: vec![
                ContractSetlistItem {
                    kind: ContractItemKind::Response,
                    title: Some("Sung response".into()),
                    ..Default::default()
                },
                ContractSetlistItem {
                    kind: ContractItemKind::Media,
                    title: Some("Welcome slide".into()),
                    ..Default::default()
                },
                ContractSetlistItem {
                    kind: ContractItemKind::Gap,
                    title: Some("Transition".into()),
                    notes: Some("30s of silence".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let specs = bulletin_from_contract(plan).unwrap();
        assert_eq!(specs[0].kind, "liturgy", "response → liturgy");
        assert_eq!(data(&specs[0])["role"], "liturgy");
        assert_eq!(specs[1].kind, "image", "media → image");
        // gap → text (the `other` fallback), nothing dropped.
        assert_eq!(specs[2].kind, "text");
        let gap = data(&specs[2]);
        assert_eq!(gap["title"], "Transition");
        assert_eq!(gap["text"], "30s of silence", "notes become the body");
    }

    #[test]
    fn unknown_future_kind_falls_back_to_text() {
        // A newer SundayPlan emits a kind this build doesn't model.
        let json = r#"{ "items": [ { "position": 0, "kind": "drama_sketch", "title": "Skit" } ] }"#;
        let plan: ContractServicePlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.items[0].kind, ContractItemKind::Custom);
        let specs = bulletin_from_contract(plan).unwrap();
        assert_eq!(specs[0].kind, "text");
        assert_eq!(data(&specs[0])["title"], "Skit");
    }

    #[test]
    fn notes_become_the_body_for_text_carrying_kinds() {
        let plan = ContractServicePlan {
            items: vec![ContractSetlistItem {
                kind: ContractItemKind::Announcement,
                title: Some("Coffee".into()),
                notes: Some("In the hall after the service".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = bulletin_from_contract(plan).unwrap();
        assert_eq!(specs[0].kind, "announcement");
        assert_eq!(data(&specs[0])["text"], "In the hall after the service");
    }

    #[test]
    fn empty_plan_is_rejected_via_build_bulletin() {
        let err = bulletin_from_contract(ContractServicePlan::default()).unwrap_err();
        assert!(matches!(err, crate::error::AppError::Validation(_)));
    }

    #[test]
    fn song_ref_falls_back_to_local_id_when_no_suite_id() {
        let plan = ContractServicePlan {
            items: vec![ContractSetlistItem {
                kind: ContractItemKind::Song,
                title: Some("Local hymn".into()),
                song_ref: Some(ContractSongRef {
                    local_id: Some("song-local-7".into()),
                    tono_work_id: Some("TONO-42".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = bulletin_from_contract(plan).unwrap();
        let d = data(&specs[0]);
        assert_eq!(d["songId"], "song-local-7", "falls back to local id");
        assert_eq!(d["tonoWorkId"], "TONO-42");
    }

    #[test]
    fn header_omitted_when_service_has_no_name_or_date() {
        let plan = ContractServicePlan {
            service: ContractServiceRef::default(),
            items: vec![ContractSetlistItem {
                kind: ContractItemKind::Prayer,
                ..Default::default()
            }],
        };
        let specs = bulletin_from_contract(plan).unwrap();
        assert_eq!(specs.len(), 1, "no header block without metadata");
        assert_eq!(specs[0].kind, "liturgy");
    }
}
