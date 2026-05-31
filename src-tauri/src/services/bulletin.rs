//! Pure "service program / bulletin from a ServicePlan" generator — the
//! Plan/Song → Paper bridge.
//!
//! This is the FORWARD direction (intent/data → block tree) for one very
//! concrete intent: a church has planned a service in SundayPlan (an ordered
//! list of items — welcome, songs, scripture readings, the sermon,
//! announcements, the offering, the benediction…) and wants a printable
//! program out of it in one click.
//!
//! The function here is **pure**: it takes a canonical [`ServicePlan`] value
//! (deserialised from JSON the caller supplies — so this module needs NO
//! cross-repo dependency on SundayPlan) and returns an ordered list of
//! [`BlockSpec`]s. Each spec is exactly what the `block` repository's
//! `create(document_id, parent_id, kind, data)` wants: a `kind` string that the
//! app already renders (`heading` / `song` / `scripture` / `liturgy` /
//! `announcement` / `text`) plus a `data` JSON payload. The caller persists the
//! specs in order as top-level blocks of a fresh `program` document.
//!
//! Because it is pure it is fully unit-tested without a database, the `pdf`
//! feature, or any hardware — same posture as `pdf::plan`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// A reference to a song in the catalog, carried through from the plan so the
/// generated block can later be re-bound to the live `song` row / lyrics.
///
/// Mirrors `sunday-contracts`; converge once published.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[ts(export, export_to = "../../src/lib/bindings/SongRef.ts")]
pub struct SongRef {
    /// SundaySong / catalog id, if the plan knew one. Carried verbatim so the
    /// block can relink to the local catalog row.
    #[serde(default)]
    pub song_id: Option<String>,
    /// TONO work id — the Nordic rights identifier. Carried "from day one" per
    /// the product principle even though we don't print it.
    #[serde(default)]
    pub tono_work_id: Option<String>,
    /// Author / composer, if known.
    #[serde(default)]
    pub author: Option<String>,
    /// Hymnal / songbook number (e.g. "N13 097"), printed next to the title.
    #[serde(default)]
    pub number: Option<String>,
}

/// A reference to a scripture passage (e.g. "John 3:16-21").
///
/// Mirrors `sunday-contracts`; converge once published.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[ts(export, export_to = "../../src/lib/bindings/ScriptureRef.ts")]
pub struct ScriptureRef {
    /// Bible book, e.g. "John".
    #[serde(default)]
    pub book: Option<String>,
    /// Chapter / verse reference, e.g. "3:16-21".
    #[serde(default)]
    pub reference: Option<String>,
    /// Translation / version, e.g. "NRSV", "Bibel 2011".
    #[serde(default)]
    pub translation: Option<String>,
}

/// The kind of a single ordered item in a service plan.
///
/// Mirrors `sunday-contracts`; converge once published. Unknown / future kinds
/// deserialise to [`SetlistItemKind::Other`] (via `#[serde(other)]`) so a
/// newer SundayPlan can add item kinds without breaking this generator — they
/// fall back to a plain text block rather than being dropped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/lib/bindings/SetlistItemKind.ts")]
pub enum SetlistItemKind {
    /// Opening welcome / greeting.
    Welcome,
    /// A song / hymn.
    Song,
    /// A scripture reading.
    Scripture,
    /// The sermon / message.
    Sermon,
    /// A spoken or read liturgical element (creed, prayer, confession…).
    Liturgy,
    /// A prayer (intercession, the Lord's Prayer…).
    Prayer,
    /// An announcement / notice.
    Announcement,
    /// The offering / collection.
    Offering,
    /// The closing blessing / benediction.
    Benediction,
    /// Anything SundayPlan adds later that we don't yet model. The default so
    /// `SetlistItem` can derive `Default`, and the fallback for unknown JSON.
    #[serde(other)]
    #[default]
    Other,
}

/// One ordered item in a service plan.
///
/// Mirrors `sunday-contracts`; converge once published.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[ts(export, export_to = "../../src/lib/bindings/SetlistItem.ts")]
pub struct SetlistItem {
    pub kind: SetlistItemKind,
    /// Item title / label, e.g. "Welcome", the song title, "First Reading".
    #[serde(default)]
    pub title: Option<String>,
    /// Free body text — sermon synopsis, announcement text, liturgy lines.
    #[serde(default)]
    pub body: Option<String>,
    /// Who leads / speaks this item, printed as a byline.
    #[serde(default)]
    pub leader: Option<String>,
    /// Present when `kind == Song`.
    #[serde(default)]
    pub song: Option<SongRef>,
    /// Present when `kind == Scripture`.
    #[serde(default)]
    pub scripture: Option<ScriptureRef>,
}

/// A whole planned service: header metadata + the ordered items.
///
/// Mirrors `sunday-contracts`; converge once published.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[ts(export, export_to = "../../src/lib/bindings/ServicePlan.ts")]
pub struct ServicePlan {
    /// Service title, e.g. "Sunday Worship".
    #[serde(default)]
    pub title: Option<String>,
    /// Congregation / parish name, printed as a subtitle.
    #[serde(default)]
    pub church: Option<String>,
    /// Human date string as the plan supplied it (e.g. "1 June 2026"). We carry
    /// it through verbatim rather than reformatting — the plan owns the locale.
    #[serde(default)]
    pub date: Option<String>,
    /// Ordered service items.
    #[serde(default)]
    pub items: Vec<SetlistItem>,
}

/// One block to create, in order, on the generated program document. The
/// caller maps each to `BlockRepo::create(document_id, None, kind, data)`.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../src/lib/bindings/BlockSpec.ts")]
pub struct BlockSpec {
    /// Block kind the app already renders (`heading`, `song`, `scripture`,
    /// `liturgy`, `announcement`, `text`).
    pub kind: String,
    /// Kind-specific payload, serialised as a JSON string (exactly what the
    /// `block` table stores in its `data` column).
    pub data: String,
}

impl BlockSpec {
    fn new(kind: &str, data: serde_json::Value) -> AppResult<Self> {
        Ok(BlockSpec {
            kind: kind.to_string(),
            // `Value` always serialises, but keep the `?` so we mirror the
            // repo's "data must be valid JSON" contract end to end.
            data: serde_json::to_string(&data)?,
        })
    }
}

/// Map a canonical [`ServicePlan`] into Paper's document block model: an
/// ordered list of [`BlockSpec`]s ready to persist as top-level program blocks.
///
/// The shape is deterministic and chosen per item kind:
///
/// - A leading `heading` block carries the service title, church and date
///   (skipped entirely if the plan has none of the three).
/// - `welcome` / `liturgy` / `prayer` / `offering` / `benediction` →
///   `liturgy` blocks (a labelled, optionally-bylined spoken element).
/// - `song` → `song` block carrying title + the full [`SongRef`] (song_id,
///   tono_work_id, author, number) so it can relink to the catalog.
/// - `scripture` → `scripture` block carrying the [`ScriptureRef`] and any
///   read text.
/// - `sermon` → `heading` block (the sermon is a section header on a printed
///   program; the manuscript itself isn't printed).
/// - `announcement` → `announcement` block.
/// - `other` / unknown future kinds → a plain `text` block so nothing is lost.
///
/// Errors only if the plan has no items at all — an empty program is a user
/// mistake worth surfacing, matching how the repos validate required input.
pub fn build_bulletin(plan: &ServicePlan) -> AppResult<Vec<BlockSpec>> {
    if plan.items.is_empty() {
        return Err(AppError::Validation(
            "a service plan needs at least one item to make a program".into(),
        ));
    }

    let mut specs = Vec::with_capacity(plan.items.len() + 1);

    // Leading header block, only when there is real (non-blank) metadata.
    let (title, subtitle, date) = (opt(&plan.title), opt(&plan.church), opt(&plan.date));
    if title.is_some() || subtitle.is_some() || date.is_some() {
        specs.push(BlockSpec::new(
            "heading",
            serde_json::json!({
                "role": "service-title",
                "title": title,
                "subtitle": subtitle,
                "date": date,
            }),
        )?);
    }

    for item in &plan.items {
        specs.push(block_for_item(item)?);
    }

    Ok(specs)
}

/// Map a single item to its block. Each branch picks the most faithful block
/// kind and carries the item's title / leader / refs through.
fn block_for_item(item: &SetlistItem) -> AppResult<BlockSpec> {
    match item.kind {
        SetlistItemKind::Song => {
            let song = item.song.clone().unwrap_or_default();
            BlockSpec::new(
                "song",
                serde_json::json!({
                    "title": opt(&item.title),
                    "leader": opt(&item.leader),
                    "songId": opt(&song.song_id),
                    "tonoWorkId": opt(&song.tono_work_id),
                    "author": opt(&song.author),
                    "number": opt(&song.number),
                }),
            )
        }
        SetlistItemKind::Scripture => {
            let s = item.scripture.clone().unwrap_or_default();
            BlockSpec::new(
                "scripture",
                serde_json::json!({
                    "title": opt(&item.title),
                    "reader": opt(&item.leader),
                    "book": opt(&s.book),
                    "reference": opt(&s.reference),
                    "translation": opt(&s.translation),
                    "text": opt(&item.body),
                }),
            )
        }
        SetlistItemKind::Sermon => BlockSpec::new(
            "heading",
            serde_json::json!({
                "role": "sermon",
                "title": opt(&item.title).unwrap_or_else(|| "Sermon".to_string()),
                "preacher": opt(&item.leader),
                "synopsis": opt(&item.body),
            }),
        ),
        SetlistItemKind::Announcement => BlockSpec::new(
            "announcement",
            serde_json::json!({
                "title": opt(&item.title),
                "text": opt(&item.body),
            }),
        ),
        SetlistItemKind::Welcome
        | SetlistItemKind::Liturgy
        | SetlistItemKind::Prayer
        | SetlistItemKind::Offering
        | SetlistItemKind::Benediction => BlockSpec::new(
            "liturgy",
            serde_json::json!({
                "role": liturgy_role(item.kind),
                "title": opt(&item.title).or_else(|| Some(default_label(item.kind).to_string())),
                "leader": opt(&item.leader),
                "text": opt(&item.body),
            }),
        ),
        SetlistItemKind::Other => BlockSpec::new(
            "text",
            serde_json::json!({
                "title": opt(&item.title),
                "text": opt(&item.body),
            }),
        ),
    }
}

/// The `role` tag a liturgy block carries so the renderer / later styling can
/// tell a benediction from an offering even though they share a block kind.
fn liturgy_role(kind: SetlistItemKind) -> &'static str {
    match kind {
        SetlistItemKind::Welcome => "welcome",
        SetlistItemKind::Prayer => "prayer",
        SetlistItemKind::Offering => "offering",
        SetlistItemKind::Benediction => "benediction",
        // Plain liturgy and anything routed here without its own role.
        _ => "liturgy",
    }
}

/// A sensible default printed label when the plan item carried no title.
fn default_label(kind: SetlistItemKind) -> &'static str {
    match kind {
        SetlistItemKind::Welcome => "Welcome",
        SetlistItemKind::Prayer => "Prayer",
        SetlistItemKind::Offering => "Offering",
        SetlistItemKind::Benediction => "Benediction",
        _ => "Liturgy",
    }
}

/// Normalise an optional, possibly-blank string to `Some(trimmed)` / `None`.
/// Blank-but-present fields in a plan shouldn't print as empty lines.
fn opt(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a spec's `data` back into a `serde_json::Value` for assertions.
    fn data(spec: &BlockSpec) -> serde_json::Value {
        serde_json::from_str(&spec.data).expect("spec data is valid JSON")
    }

    /// A representative plan touching every item kind.
    fn representative_plan() -> ServicePlan {
        ServicePlan {
            title: Some("Sunday Worship".into()),
            church: Some("St. Olav's".into()),
            date: Some("1 June 2026".into()),
            items: vec![
                SetlistItem {
                    kind: SetlistItemKind::Welcome,
                    title: Some("Welcome & Greeting".into()),
                    leader: Some("Pastor Anne".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Song,
                    title: Some("Holy, Holy, Holy".into()),
                    song: Some(SongRef {
                        song_id: Some("song-123".into()),
                        tono_work_id: Some("TONO-999".into()),
                        author: Some("Reginald Heber".into()),
                        number: Some("N13 097".into()),
                    }),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Scripture,
                    title: Some("First Reading".into()),
                    leader: Some("Lay reader".into()),
                    body: Some("For God so loved the world…".into()),
                    scripture: Some(ScriptureRef {
                        book: Some("John".into()),
                        reference: Some("3:16-21".into()),
                        translation: Some("NRSV".into()),
                    }),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Sermon,
                    title: Some("The Light of the World".into()),
                    leader: Some("Pastor Anne".into()),
                    body: Some("On living in the light.".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Liturgy,
                    title: Some("Apostles' Creed".into()),
                    body: Some("I believe in God…".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Prayer,
                    title: Some("Prayers of the People".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Announcement,
                    title: Some("Coffee after the service".into()),
                    body: Some("Join us in the hall.".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Offering,
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Benediction,
                    leader: Some("Pastor Anne".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Other,
                    title: Some("Postlude".into()),
                    body: Some("Organ voluntary".into()),
                    ..Default::default()
                },
            ],
        }
    }

    #[test]
    fn empty_plan_is_rejected() {
        let plan = ServicePlan::default();
        let err = build_bulletin(&plan).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn header_block_leads_and_carries_metadata() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        // header + one block per item.
        assert_eq!(specs.len(), plan.items.len() + 1);
        let head = &specs[0];
        assert_eq!(head.kind, "heading");
        let d = data(head);
        assert_eq!(d["role"], "service-title");
        assert_eq!(d["title"], "Sunday Worship");
        assert_eq!(d["subtitle"], "St. Olav's");
        assert_eq!(d["date"], "1 June 2026");
    }

    #[test]
    fn header_is_omitted_when_no_metadata() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Welcome,
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(specs.len(), 1, "no header block when plan has no metadata");
        assert_eq!(specs[0].kind, "liturgy");
    }

    #[test]
    fn order_is_preserved() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        // Drop the header, then the kinds should follow the plan order.
        let kinds: Vec<&str> = specs[1..].iter().map(|s| s.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "liturgy",      // welcome
                "song",         // song
                "scripture",    // scripture
                "heading",      // sermon
                "liturgy",      // liturgy (creed)
                "liturgy",      // prayer
                "announcement", // announcement
                "liturgy",      // offering
                "liturgy",      // benediction
                "text",         // other
            ]
        );
    }

    #[test]
    fn song_block_carries_refs() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let song = specs.iter().find(|s| s.kind == "song").unwrap();
        let d = data(song);
        assert_eq!(d["title"], "Holy, Holy, Holy");
        assert_eq!(d["songId"], "song-123");
        assert_eq!(d["tonoWorkId"], "TONO-999");
        assert_eq!(d["author"], "Reginald Heber");
        assert_eq!(d["number"], "N13 097");
    }

    #[test]
    fn song_without_ref_still_produces_song_block() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Song,
                title: Some("Untitled Hymn".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        let d = data(&specs[0]);
        assert_eq!(specs[0].kind, "song");
        assert_eq!(d["title"], "Untitled Hymn");
        assert!(d["songId"].is_null(), "missing ref → null, not crash");
        assert!(d["tonoWorkId"].is_null());
    }

    #[test]
    fn scripture_block_carries_reference_and_text() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let scr = specs.iter().find(|s| s.kind == "scripture").unwrap();
        let d = data(scr);
        assert_eq!(d["title"], "First Reading");
        assert_eq!(d["reader"], "Lay reader");
        assert_eq!(d["book"], "John");
        assert_eq!(d["reference"], "3:16-21");
        assert_eq!(d["translation"], "NRSV");
        assert_eq!(d["text"], "For God so loved the world…");
    }

    #[test]
    fn sermon_becomes_heading_with_preacher_and_synopsis() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        // The second heading is the sermon (the first is the service title).
        let sermon = specs
            .iter()
            .find(|s| s.kind == "heading" && data(s)["role"] == "sermon")
            .unwrap();
        let d = data(sermon);
        assert_eq!(d["title"], "The Light of the World");
        assert_eq!(d["preacher"], "Pastor Anne");
        assert_eq!(d["synopsis"], "On living in the light.");
    }

    #[test]
    fn sermon_without_title_defaults_label() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Sermon,
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(data(&specs[0])["title"], "Sermon");
    }

    #[test]
    fn announcement_block_carries_text() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let ann = specs.iter().find(|s| s.kind == "announcement").unwrap();
        let d = data(ann);
        assert_eq!(d["title"], "Coffee after the service");
        assert_eq!(d["text"], "Join us in the hall.");
    }

    #[test]
    fn liturgy_kinds_carry_distinct_roles() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let roles: Vec<String> = specs
            .iter()
            .filter(|s| s.kind == "liturgy")
            .map(|s| data(s)["role"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            roles,
            vec!["welcome", "liturgy", "prayer", "offering", "benediction"]
        );
    }

    #[test]
    fn liturgy_without_title_gets_default_label() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        // The offering item had no title → defaults to "Offering".
        let offering = specs
            .iter()
            .find(|s| s.kind == "liturgy" && data(s)["role"] == "offering")
            .unwrap();
        assert_eq!(data(offering)["title"], "Offering");
        // Benediction had no title either.
        let ben = specs
            .iter()
            .find(|s| s.kind == "liturgy" && data(s)["role"] == "benediction")
            .unwrap();
        assert_eq!(data(ben)["title"], "Benediction");
        assert_eq!(data(ben)["leader"], "Pastor Anne");
    }

    #[test]
    fn other_kind_falls_back_to_text_block() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let last = specs.last().unwrap();
        assert_eq!(last.kind, "text");
        let d = data(last);
        assert_eq!(d["title"], "Postlude");
        assert_eq!(d["text"], "Organ voluntary");
    }

    #[test]
    fn blank_strings_normalise_to_null() {
        let plan = ServicePlan {
            title: Some("   ".into()),
            items: vec![SetlistItem {
                kind: SetlistItemKind::Announcement,
                title: Some("".into()),
                body: Some("  real text  ".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        // Blank title everywhere → no header block at all.
        assert_eq!(specs.len(), 1);
        let d = data(&specs[0]);
        assert!(d["title"].is_null(), "blank title → null");
        assert_eq!(d["text"], "real text", "body is trimmed");
    }

    #[test]
    fn unknown_item_kind_deserialises_to_other() {
        // A future SundayPlan adds a "creed" kind we don't model yet.
        let json = r#"{
            "items": [
                { "kind": "totally_new_kind", "title": "Mystery", "body": "x" }
            ]
        }"#;
        let plan: ServicePlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.items[0].kind, SetlistItemKind::Other);
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(specs[0].kind, "text", "unknown kind survives as text");
        assert_eq!(data(&specs[0])["title"], "Mystery");
    }

    #[test]
    fn full_plan_deserialises_from_canonical_json() {
        // Proves the local mirror round-trips the JSON a real SundayPlan emits.
        let json = r#"{
            "title": "Morgenmesse",
            "church": "Domkirken",
            "date": "1. juni 2026",
            "items": [
                { "kind": "welcome", "title": "Velkommen", "leader": "Kapellan" },
                {
                    "kind": "song",
                    "title": "Deg være ære",
                    "song": { "song_id": "s1", "tono_work_id": "T1", "number": "N13 197" }
                },
                {
                    "kind": "scripture",
                    "title": "Lesning",
                    "scripture": { "book": "Johannes", "reference": "20:1-10" }
                },
                { "kind": "sermon", "title": "Oppstandelsen", "leader": "Biskop" },
                { "kind": "benediction" }
            ]
        }"#;
        let plan: ServicePlan = serde_json::from_str(json).unwrap();
        let specs = build_bulletin(&plan).unwrap();
        // header + 5 items.
        assert_eq!(specs.len(), 6);
        assert_eq!(specs[0].kind, "heading");
        assert_eq!(data(&specs[0])["title"], "Morgenmesse");
        assert_eq!(specs[2].kind, "song");
        assert_eq!(data(&specs[2])["number"], "N13 197");
        assert_eq!(data(&specs[2])["tonoWorkId"], "T1");
        assert_eq!(specs[5].kind, "liturgy");
        assert_eq!(data(&specs[5])["role"], "benediction");
        assert_eq!(data(&specs[5])["title"], "Benediction");
    }

    #[test]
    fn every_spec_data_is_valid_json() {
        // The block repo rejects non-JSON data; guarantee we never produce any.
        let specs = build_bulletin(&representative_plan()).unwrap();
        for spec in &specs {
            assert!(!spec.kind.trim().is_empty(), "kind is never blank");
            serde_json::from_str::<serde_json::Value>(&spec.data).expect("data must be valid JSON");
        }
    }
}
