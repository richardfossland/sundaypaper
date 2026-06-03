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
    /// The song's verses, in singing order — one entry per verse (a verse may
    /// itself span several lines). Carried when the plan is bound to the song
    /// catalog so the program prints the actual words, not just the heading.
    /// Empty / absent means a bare reference (heading only), as before.
    #[serde(default)]
    pub verses: Vec<String>,
    /// The refrain / chorus, printed once after the first verse. `None` when the
    /// song has no refrain.
    #[serde(default)]
    pub refrain: Option<String>,
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

/// A reference to an asset / image carried from the plan so the generated block
/// can later re-bind to the local asset library row by id, or fall back to a
/// URL the plan supplied.
///
/// Mirrors `sunday-contracts`; converge once published.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[ts(export, export_to = "../../src/lib/bindings/AssetRef.ts")]
pub struct AssetRef {
    /// Asset-library id, if the plan knew one.
    #[serde(default)]
    pub asset_id: Option<String>,
    /// A URL / path the plan supplied (e.g. a poster the planner attached).
    #[serde(default)]
    pub url: Option<String>,
    /// Alt text / caption for the image.
    #[serde(default)]
    pub caption: Option<String>,
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
    /// A spoken or read liturgical element (general liturgical text).
    Liturgy,
    /// A creed / confession of faith (Apostles', Nicene…). A liturgical element
    /// distinct enough to print with its own role so it can be styled apart.
    Creed,
    /// A prayer (intercession, the Lord's Prayer…).
    Prayer,
    /// Holy Communion / the Eucharist / Lord's Supper.
    Communion,
    /// Instrumental music with no congregational part (prelude, postlude,
    /// offertory voluntary) — printed as a titled note, not a song block.
    Music,
    /// An announcement / notice.
    Announcement,
    /// The offering / collection.
    Offering,
    /// A standalone image / poster the planner attached (e.g. a seasonal
    /// banner). Carries an [`AssetRef`].
    Image,
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
    /// A clock time the plan attached (e.g. "10:30"), printed in the margin so
    /// a printed order-of-service can double as a run sheet.
    #[serde(default)]
    pub time: Option<String>,
    /// Rights line for printed song lyrics (e.g. a CCLI / copyright credit).
    /// Carried so a song block can print the credit the licence requires — the
    /// Nordic TONO/CCLI reality from the product principles.
    #[serde(default)]
    pub copyright: Option<String>,
    /// When true the renderer should start this item on a fresh page (e.g. a
    /// full-page hymn sheet). Defaults to false.
    #[serde(default)]
    pub page_break: bool,
    /// Present when `kind == Song`.
    #[serde(default)]
    pub song: Option<SongRef>,
    /// Present when `kind == Scripture`.
    #[serde(default)]
    pub scripture: Option<ScriptureRef>,
    /// Present when `kind == Image`.
    #[serde(default)]
    pub asset: Option<AssetRef>,
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

/// The fillable-form block kinds the FormBuilder (Phase 7.2) adds on top of the
/// program block kinds. Unlike the program kinds these are not derived from a
/// [`ServicePlan`]; a volunteer composes them by hand (a signup sheet, an
/// attendance form, a donation card). Each maps to one block kind the layout
/// engine renders as a *printed* field — a labelled rule or box a person fills
/// in by hand — so member/form data never has to leave the machine to fill the
/// document, honouring the privacy promise (see CLAUDE.md).
///
/// The variants carry only layout-shaping hints (label, placeholder, width);
/// the actual answers are never modelled here — they're written on the printed
/// page. [`FormField::into_spec`] turns a variant into the same `(kind, data)`
/// [`BlockSpec`] the program generator emits, so forms persist through the very
/// same `BlockRepo::create` path and render through the same Typst builder.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
#[ts(export, export_to = "../../src/lib/bindings/FormField.ts")]
pub enum FormField {
    /// A fillable text line: a label, an optional faint placeholder `hint`, and
    /// an optional `width` keyword (`full` / `half` / `third` / `quarter`).
    TextField {
        label: String,
        #[serde(default)]
        hint: Option<String>,
        #[serde(default)]
        width: Option<String>,
    },
    /// A tick box with a label (opt-ins, attendance, consent lines).
    CheckBox { label: String },
    /// A wide rule to sign on, the label printed small beneath it.
    Signature {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        width: Option<String>,
    },
}

impl FormField {
    /// The block `kind` string this field renders as — the same dispatch keys
    /// `layout::markup` matches on (`form_field` / `checkbox` / `signature`).
    pub fn block_kind(&self) -> &'static str {
        match self {
            FormField::TextField { .. } => "form_field",
            FormField::CheckBox { .. } => "checkbox",
            FormField::Signature { .. } => "signature",
        }
    }

    /// Lower a form field to the canonical `(kind, data)` [`BlockSpec`] the rest
    /// of the pipeline persists and renders. Blank-but-present strings normalise
    /// to absent so the printed field stays clean — same `opt` rule the program
    /// generator uses.
    pub fn into_spec(self) -> AppResult<BlockSpec> {
        let kind = self.block_kind();
        let data = match self {
            FormField::TextField { label, hint, width } => serde_json::json!({
                "label": opt(&Some(label)),
                "hint": opt(&hint),
                "width": opt(&width),
            }),
            FormField::CheckBox { label } => serde_json::json!({
                "label": opt(&Some(label)),
            }),
            FormField::Signature { label, width } => serde_json::json!({
                "label": opt(&label),
                "width": opt(&width),
            }),
        };
        BlockSpec::new(kind, data)
    }
}

/// Map a canonical [`ServicePlan`] into Paper's document block model: an
/// ordered list of [`BlockSpec`]s ready to persist as top-level program blocks.
///
/// The shape is deterministic and chosen per item kind:
///
/// - A leading `heading` block carries the service title, church and date
///   (skipped entirely if the plan has none of the three).
/// - `welcome` / `liturgy` / `creed` / `prayer` / `communion` / `offering` /
///   `benediction` → `liturgy` blocks (a labelled, optionally-bylined spoken
///   element) each carrying a distinct `role` so the renderer can style them
///   apart.
/// - `song` → `song` block carrying title + the full [`SongRef`] (song_id,
///   tono_work_id, author, number) plus any `copyright` credit line.
/// - `music` → `music` block (instrumental: title + leader, no lyrics).
/// - `scripture` → `scripture` block carrying the [`ScriptureRef`] and any
///   read text.
/// - `sermon` → `heading` block (the sermon is a section header on a printed
///   program; the manuscript itself isn't printed).
/// - `announcement` → `announcement` block.
/// - `image` → `image` block carrying the [`AssetRef`] (asset id / url / caption).
/// - `other` / unknown future kinds → a plain `text` block so nothing is lost.
///
/// Every block additionally carries any `time` (margin run-sheet clock) and a
/// `pageBreak` flag the item set, so cross-cutting layout hints survive
/// regardless of kind.
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
/// kind and carries the item's title / leader / refs through; the cross-cutting
/// `time` and `pageBreak` hints are then merged onto every block so they
/// survive no matter the kind.
fn block_for_item(item: &SetlistItem) -> AppResult<BlockSpec> {
    let (kind, mut data) = match item.kind {
        SetlistItemKind::Song => {
            let song = item.song.clone().unwrap_or_default();
            (
                "song",
                serde_json::json!({
                    "title": opt(&item.title),
                    "leader": opt(&item.leader),
                    "songId": opt(&song.song_id),
                    "tonoWorkId": opt(&song.tono_work_id),
                    "author": opt(&song.author),
                    "number": opt(&song.number),
                    // Full lyrics when the plan bound them; the markup builder
                    // numbers verses and indents the refrain. Blank entries are
                    // dropped downstream, so an empty list prints nothing.
                    "verses": song.verses,
                    "refrain": opt(&song.refrain),
                    // Rights credit the licence may require us to print.
                    "copyright": opt(&item.copyright),
                }),
            )
        }
        SetlistItemKind::Music => (
            "music",
            serde_json::json!({
                "title": opt(&item.title),
                "leader": opt(&item.leader),
                "text": opt(&item.body),
            }),
        ),
        SetlistItemKind::Scripture => {
            let s = item.scripture.clone().unwrap_or_default();
            (
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
        SetlistItemKind::Sermon => (
            "heading",
            serde_json::json!({
                "role": "sermon",
                "title": opt(&item.title).unwrap_or_else(|| "Sermon".to_string()),
                "preacher": opt(&item.leader),
                "synopsis": opt(&item.body),
            }),
        ),
        SetlistItemKind::Announcement => (
            "announcement",
            serde_json::json!({
                "title": opt(&item.title),
                "text": opt(&item.body),
            }),
        ),
        SetlistItemKind::Image => {
            let a = item.asset.clone().unwrap_or_default();
            (
                "image",
                serde_json::json!({
                    "title": opt(&item.title),
                    "assetId": opt(&a.asset_id),
                    "url": opt(&a.url),
                    "caption": opt(&a.caption).or_else(|| opt(&item.body)),
                }),
            )
        }
        SetlistItemKind::Welcome
        | SetlistItemKind::Liturgy
        | SetlistItemKind::Creed
        | SetlistItemKind::Prayer
        | SetlistItemKind::Communion
        | SetlistItemKind::Offering
        | SetlistItemKind::Benediction => (
            "liturgy",
            serde_json::json!({
                "role": liturgy_role(item.kind),
                "title": opt(&item.title).or_else(|| Some(default_label(item.kind).to_string())),
                "leader": opt(&item.leader),
                "text": opt(&item.body),
            }),
        ),
        SetlistItemKind::Other => (
            "text",
            serde_json::json!({
                "title": opt(&item.title),
                "text": opt(&item.body),
            }),
        ),
    };

    merge_hints(&mut data, item);
    BlockSpec::new(kind, data)
}

/// Attach the cross-cutting layout hints — the run-sheet `time` and the
/// `pageBreak` flag — onto a block's payload. `time` is only set when present
/// so blocks without a clock stay free of a null key; `pageBreak` is always
/// written (a plain bool) so the renderer never has to treat "absent" specially.
fn merge_hints(data: &mut serde_json::Value, item: &SetlistItem) {
    let obj = data
        .as_object_mut()
        .expect("block payloads are always JSON objects");
    if let Some(time) = opt(&item.time) {
        obj.insert("time".into(), serde_json::Value::String(time));
    }
    obj.insert("pageBreak".into(), serde_json::Value::Bool(item.page_break));
}

/// The `role` tag a liturgy block carries so the renderer / later styling can
/// tell a benediction from an offering even though they share a block kind.
fn liturgy_role(kind: SetlistItemKind) -> &'static str {
    match kind {
        SetlistItemKind::Welcome => "welcome",
        SetlistItemKind::Creed => "creed",
        SetlistItemKind::Prayer => "prayer",
        SetlistItemKind::Communion => "communion",
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
        SetlistItemKind::Creed => "Creed",
        SetlistItemKind::Prayer => "Prayer",
        SetlistItemKind::Communion => "Holy Communion",
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
                        verses: vec![
                            "Holy, holy, holy! Lord God Almighty!".into(),
                            "Holy, holy, holy! All the saints adore thee".into(),
                        ],
                        refrain: None,
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
                    kind: SetlistItemKind::Creed,
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
                    kind: SetlistItemKind::Communion,
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
                    kind: SetlistItemKind::Image,
                    title: Some("Easter banner".into()),
                    asset: Some(AssetRef {
                        asset_id: Some("asset-7".into()),
                        url: Some("https://x/banner.png".into()),
                        caption: Some("He is risen".into()),
                    }),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Benediction,
                    leader: Some("Pastor Anne".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Music,
                    title: Some("Postlude".into()),
                    body: Some("Organ voluntary".into()),
                    leader: Some("Organist".into()),
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Other,
                    title: Some("Notices".into()),
                    body: Some("misc".into()),
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
                "liturgy",      // creed
                "liturgy",      // prayer
                "liturgy",      // communion
                "announcement", // announcement
                "liturgy",      // offering
                "image",        // image
                "liturgy",      // benediction
                "music",        // music
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
    fn song_block_carries_verses_and_refrain() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Song,
                title: Some("Amazing Grace".into()),
                song: Some(SongRef {
                    verses: vec!["Amazing grace, how sweet".into(), "'Twas grace".into()],
                    refrain: Some("Praise be".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let d = data(&build_bulletin(&plan).unwrap()[0]);
        let verses = d["verses"].as_array().expect("verses array carried");
        assert_eq!(verses.len(), 2);
        assert_eq!(verses[0], "Amazing grace, how sweet");
        assert_eq!(d["refrain"], "Praise be");
    }

    #[test]
    fn song_block_without_lyrics_carries_empty_verses_and_null_refrain() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Song,
                title: Some("Bare".into()),
                song: Some(SongRef {
                    number: Some("N1".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let d = data(&build_bulletin(&plan).unwrap()[0]);
        assert_eq!(d["verses"].as_array().expect("verses present").len(), 0);
        assert!(d["refrain"].is_null());
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
            vec![
                "welcome",
                "creed",
                "prayer",
                "communion",
                "offering",
                "benediction"
            ]
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
        assert_eq!(d["title"], "Notices");
        assert_eq!(d["text"], "misc");
    }

    #[test]
    fn music_becomes_music_block_not_song() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let music = specs.iter().find(|s| s.kind == "music").unwrap();
        // Instrumental → no songId/tonoWorkId keys, but title + leader + note.
        let d = data(music);
        assert_eq!(d["title"], "Postlude");
        assert_eq!(d["leader"], "Organist");
        assert_eq!(d["text"], "Organ voluntary");
        assert!(d.get("songId").is_none(), "music is not a song block");
    }

    #[test]
    fn image_block_carries_asset_ref() {
        let plan = representative_plan();
        let specs = build_bulletin(&plan).unwrap();
        let img = specs.iter().find(|s| s.kind == "image").unwrap();
        let d = data(img);
        assert_eq!(d["title"], "Easter banner");
        assert_eq!(d["assetId"], "asset-7");
        assert_eq!(d["url"], "https://x/banner.png");
        assert_eq!(d["caption"], "He is risen");
    }

    #[test]
    fn image_caption_falls_back_to_body() {
        // No explicit caption on the asset → the item body becomes the caption.
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Image,
                body: Some("A photo from last week".into()),
                asset: Some(AssetRef {
                    url: Some("u".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(specs[0].kind, "image");
        assert_eq!(data(&specs[0])["caption"], "A photo from last week");
    }

    #[test]
    fn communion_and_creed_get_default_labels_and_roles() {
        let plan = ServicePlan {
            items: vec![
                SetlistItem {
                    kind: SetlistItemKind::Communion,
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Creed,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(data(&specs[0])["role"], "communion");
        assert_eq!(data(&specs[0])["title"], "Holy Communion");
        assert_eq!(data(&specs[1])["role"], "creed");
        assert_eq!(data(&specs[1])["title"], "Creed");
    }

    #[test]
    fn song_block_carries_copyright_credit() {
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Song,
                title: Some("Amazing Grace".into()),
                copyright: Some("Public Domain / arr. © 2020 X (CCLI 123)".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        assert_eq!(
            data(&specs[0])["copyright"],
            "Public Domain / arr. © 2020 X (CCLI 123)"
        );
    }

    #[test]
    fn time_hint_is_present_only_when_set_and_page_break_always() {
        let plan = ServicePlan {
            items: vec![
                SetlistItem {
                    kind: SetlistItemKind::Welcome,
                    time: Some(" 10:30 ".into()),
                    page_break: true,
                    ..Default::default()
                },
                SetlistItem {
                    kind: SetlistItemKind::Prayer,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let specs = build_bulletin(&plan).unwrap();
        let first = data(&specs[0]);
        assert_eq!(first["time"], "10:30", "time is trimmed");
        assert_eq!(first["pageBreak"], true);
        let second = data(&specs[1]);
        assert!(
            second.get("time").is_none(),
            "no time key when the item had none"
        );
        assert_eq!(
            second["pageBreak"], false,
            "pageBreak is always written, defaulting to false"
        );
    }

    #[test]
    fn hints_merge_onto_every_kind_including_song_and_image() {
        // The hint merge must not clobber kind-specific payload.
        let plan = ServicePlan {
            items: vec![SetlistItem {
                kind: SetlistItemKind::Song,
                title: Some("Hymn".into()),
                time: Some("09:00".into()),
                page_break: true,
                song: Some(SongRef {
                    number: Some("N1".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let d = data(&build_bulletin(&plan).unwrap()[0]);
        assert_eq!(d["number"], "N1", "kind payload survives the hint merge");
        assert_eq!(d["time"], "09:00");
        assert_eq!(d["pageBreak"], true);
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

    // --- form fields (Phase 7.2) ---------------------------------------------

    #[test]
    fn text_field_lowers_to_form_field_block_with_label_hint_width() {
        let spec = FormField::TextField {
            label: "Full name".into(),
            hint: Some("First & last".into()),
            width: Some("half".into()),
        }
        .into_spec()
        .unwrap();
        assert_eq!(spec.kind, "form_field");
        let d = data(&spec);
        assert_eq!(d["label"], "Full name");
        assert_eq!(d["hint"], "First & last");
        assert_eq!(d["width"], "half");
    }

    #[test]
    fn checkbox_lowers_to_checkbox_block_with_label() {
        let spec = FormField::CheckBox {
            label: "I consent to be contacted".into(),
        }
        .into_spec()
        .unwrap();
        assert_eq!(spec.kind, "checkbox");
        assert_eq!(data(&spec)["label"], "I consent to be contacted");
    }

    #[test]
    fn signature_lowers_to_signature_block_label_optional() {
        let with = FormField::Signature {
            label: Some("Parent / guardian".into()),
            width: None,
        }
        .into_spec()
        .unwrap();
        assert_eq!(with.kind, "signature");
        assert_eq!(data(&with)["label"], "Parent / guardian");

        // A signature with no label → null label (the renderer supplies a
        // sensible default), not a crash.
        let bare = FormField::Signature {
            label: None,
            width: None,
        }
        .into_spec()
        .unwrap();
        assert!(data(&bare)["label"].is_null());
    }

    #[test]
    fn form_field_blank_strings_normalise_to_null() {
        let spec = FormField::TextField {
            label: "  E-mail  ".into(),
            hint: Some("   ".into()),
            width: Some("".into()),
        }
        .into_spec()
        .unwrap();
        let d = data(&spec);
        assert_eq!(d["label"], "E-mail", "label is trimmed");
        assert!(d["hint"].is_null(), "blank hint → null");
        assert!(d["width"].is_null(), "blank width → null");
    }

    #[test]
    fn form_field_block_kind_matches_the_variant() {
        assert_eq!(
            FormField::TextField {
                label: "x".into(),
                hint: None,
                width: None
            }
            .block_kind(),
            "form_field"
        );
        assert_eq!(
            FormField::CheckBox { label: "x".into() }.block_kind(),
            "checkbox"
        );
        assert_eq!(
            FormField::Signature {
                label: None,
                width: None
            }
            .block_kind(),
            "signature"
        );
    }

    #[test]
    fn form_field_deserialises_from_tagged_json() {
        // Mirrors what the FormBuilder UI sends: a tagged union keyed on `kind`.
        let json = r#"{ "kind": "text_field", "label": "Phone", "width": "third" }"#;
        let field: FormField = serde_json::from_str(json).unwrap();
        assert_eq!(
            field,
            FormField::TextField {
                label: "Phone".into(),
                hint: None,
                width: Some("third".into()),
            }
        );
        // And it lowers to a valid block spec.
        let spec = field.into_spec().unwrap();
        assert_eq!(spec.kind, "form_field");
        serde_json::from_str::<serde_json::Value>(&spec.data).expect("valid JSON data");
    }
}
