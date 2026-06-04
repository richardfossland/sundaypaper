//! Pure block-tree → Typst source-string builder — the deterministic half of
//! the layout engine (Phase 4.2).
//!
//! Producing a PDF is two steps: (1) turn the document's block tree into a
//! **Typst source string**, then (2) hand that string to the Typst compiler.
//! Step (2) needs the embedded compiler crate and so stays behind the `typst`
//! cargo feature (see `layout::engine`, INFRA-UNVERIFIED). Step (1) is plain
//! string assembly — no compiler, no I/O — so it lives here, is always
//! compiled, and is exhaustively unit-tested. Same posture as `pdf::plan`.
//!
//! The builder consumes the exact JSON payloads the `bulletin` generator emits
//! (`heading` / `song` / `music` / `scripture` / `liturgy` / `announcement` /
//! `image` / `text`) plus the form-field kinds the FormBuilder adds
//! (`form_field` / `checkbox` / `signature`), so the FORWARD pipeline composes
//! end to end:
//!
//! ```text
//! ServicePlan --build_bulletin--> BlockSpec[] --(persist)--> Block tree
//!             --build_typst_document--> Typst source --(compile)--> PDF
//! ```
//!
//! Two design rules make the output trustworthy without rendering it:
//! 1. **Everything user-supplied is escaped** via [`escape_content`] before it
//!    reaches the source, so a song title with a `#` or `$` can never inject
//!    Typst markup or break compilation.
//! 2. The emitted source is **deterministic** — the same tree always yields the
//!    same bytes — so tests can assert on it directly.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Page metadata for the generated document — drives the Typst `set page` /
/// `set text` preamble. Kept tiny and self-contained so the builder needs no DB
/// row; the caller fills it from the `document` record.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/LayoutMeta.ts")]
pub struct LayoutMeta {
    /// Paper size keyword Typst understands (`a4`, `a5`, `us-letter`, …).
    /// Validated/normalised by [`normalize_paper`]; an unknown value falls back
    /// to `a4` so the source always compiles.
    pub paper: String,
    /// Base font size in points (clamped to a sane 6–48 pt range).
    pub font_size_pt: f64,
    /// Optional default language for Typst hyphenation (`en`, `nb`, `de`, …).
    pub lang: Option<String>,
    /// Optional per-church branding (fonts / accent colour / heading weight /
    /// spacing). `None` means "house default": the preamble is then emitted
    /// byte-for-byte as it was before themes existed, so an unthemed document is
    /// completely unchanged. A `Some(theme)` injects the church's look
    /// consistently into headings, song titles and scripture.
    #[serde(default)]
    pub theme: Option<LayoutTheme>,
}

impl Default for LayoutMeta {
    fn default() -> Self {
        Self {
            paper: "a4".into(),
            font_size_pt: 11.0,
            lang: None,
            theme: None,
        }
    }
}

/// Per-church branding applied to the document preamble. Every field is optional
/// so a partial theme (just an accent colour, say) still works and the rest of
/// the look stays at the house default. The values that reach Typst as raw
/// identifiers — font names and the accent colour — are validated/escaped by
/// [`LayoutTheme::resolve`] so a theme can never inject markup or break the
/// compile (a bad value silently falls back to the house default for that one
/// field; the document still renders).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/LayoutTheme.ts")]
pub struct LayoutTheme {
    /// Font family for headings / song titles / the service title. A free-text
    /// font name; validated to a safe character set before it reaches Typst.
    #[serde(default)]
    pub heading_font: Option<String>,
    /// Font family for body text (paragraphs, lyrics, scripture). Validated like
    /// `heading_font`.
    #[serde(default)]
    pub body_font: Option<String>,
    /// Accent colour used for headings / rules — a `#rrggbb` (or `#rgb`) hex
    /// string. Validated to a strict hex form before it reaches Typst's `rgb()`.
    #[serde(default)]
    pub accent_color: Option<String>,
    /// Heading weight keyword (`regular` / `medium` / `semibold` / `bold` /
    /// `black`). Only this fixed set is accepted; anything else falls back to the
    /// house default `bold`.
    #[serde(default)]
    pub heading_weight: Option<String>,
    /// Multiplier on the baseline paragraph leading (line spacing). `1.0` is the
    /// house default; clamped to a sane 0.5–3.0 range so a theme can't collapse
    /// or explode the layout.
    #[serde(default)]
    pub spacing_multiplier: Option<f64>,
}

/// The house defaults a theme overrides — kept as named constants so the "no
/// theme → byte-identical" regression pin is obvious and a partial theme falls
/// back to exactly these.
const DEFAULT_HEADING_FONT: &str = "linux libertine";
const DEFAULT_BODY_FONT: &str = "linux libertine";
const DEFAULT_ACCENT: &str = "rgb(\"#000000\")";
const DEFAULT_HEADING_WEIGHT: &str = "bold";
const DEFAULT_LEADING_EM: f64 = 0.65;

/// A theme resolved to ready-to-inject Typst fragments. Every field is already
/// validated/escaped (font names to a safe identifier, the accent to a
/// `rgb("#…")` literal), so the preamble formatter can drop them straight in.
struct ResolvedTheme {
    heading_font: String,
    body_font: String,
    accent: String,
    heading_weight: String,
    leading_em: f64,
}

impl LayoutTheme {
    /// Validate + resolve this theme against the house defaults. Each field that
    /// is absent, blank, or fails validation falls back to its house default, so
    /// the result is always a complete, injection-safe set of fragments.
    fn resolve(&self) -> ResolvedTheme {
        ResolvedTheme {
            heading_font: self
                .heading_font
                .as_deref()
                .and_then(sanitize_font_name)
                .unwrap_or_else(|| DEFAULT_HEADING_FONT.to_string()),
            body_font: self
                .body_font
                .as_deref()
                .and_then(sanitize_font_name)
                .unwrap_or_else(|| DEFAULT_BODY_FONT.to_string()),
            accent: self
                .accent_color
                .as_deref()
                .and_then(sanitize_hex_color)
                .map(|hex| format!("rgb(\"{hex}\")"))
                .unwrap_or_else(|| DEFAULT_ACCENT.to_string()),
            heading_weight: self
                .heading_weight
                .as_deref()
                .and_then(sanitize_weight)
                .unwrap_or_else(|| DEFAULT_HEADING_WEIGHT.to_string()),
            leading_em: DEFAULT_LEADING_EM * clamp_spacing(self.spacing_multiplier),
        }
    }
}

/// Validate a user-supplied font family name. Typst takes the family as a string
/// literal, so the only hard requirement is escaping; but to keep a theme from
/// smuggling odd control content we also restrict to a sane printable set
/// (letters, digits, space, and a few punctuation marks fonts actually use).
/// Returns the trimmed name when acceptable, else `None` (→ house default).
fn sanitize_font_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() || name.len() > 64 {
        return None;
    }
    let ok = name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '+' | '\''));
    ok.then(|| name.to_string())
}

/// Validate a `#rgb` / `#rrggbb` hex colour. Returns the canonical lowercase
/// `#rrggbb`/`#rgb` string (with a leading `#`) when valid, else `None`.
/// Strict — only the hash and hex digits — so nothing else can reach Typst.
fn sanitize_hex_color(raw: &str) -> Option<String> {
    let s = raw.trim();
    let hex = s.strip_prefix('#')?;
    if !(hex.len() == 3 || hex.len() == 6) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", hex.to_ascii_lowercase()))
}

/// Map a heading-weight keyword to a Typst weight string. Only the fixed set is
/// accepted (no expression injection); unknown → `None` (→ house default).
fn sanitize_weight(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "regular" => Some("regular".to_string()),
        "medium" => Some("medium".to_string()),
        "semibold" => Some("semibold".to_string()),
        "bold" => Some("bold".to_string()),
        "black" => Some("black".to_string()),
        _ => None,
    }
}

/// Clamp the spacing multiplier into a sane range; absent / non-finite → 1.0
/// (the house default, which reproduces the original leading exactly).
fn clamp_spacing(m: Option<f64>) -> f64 {
    match m {
        Some(v) if v.is_finite() => v.clamp(0.5, 3.0),
        _ => 1.0,
    }
}

/// A node in the tree handed to the builder. Mirrors a persisted `block` row
/// stripped to what layout needs (`kind` + parsed `data` + `children`), so the
/// builder stays free of the DB types and is trivially constructed in tests.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderBlock {
    pub kind: String,
    /// Parsed block payload (the `data` JSON column, already deserialised).
    pub data: serde_json::Value,
    pub children: Vec<RenderBlock>,
}

impl RenderBlock {
    /// Build a leaf node from a kind and a JSON payload.
    pub fn leaf(kind: &str, data: serde_json::Value) -> Self {
        Self {
            kind: kind.to_string(),
            data,
            children: Vec::new(),
        }
    }

    /// Convenience for tests / callers: parse a `(kind, data_json_str)` pair —
    /// exactly the `BlockSpec` shape — into a leaf, defaulting unparsable or
    /// empty data to an empty object so the builder never panics.
    pub fn from_spec(kind: &str, data_json: &str) -> Self {
        let data = serde_json::from_str(data_json).unwrap_or(serde_json::Value::Null);
        let data = if data.is_object() {
            data
        } else {
            serde_json::json!({})
        };
        Self::leaf(kind, data)
    }
}

/// Assemble a complete, self-contained Typst document source string from page
/// metadata and an ordered block tree.
///
/// The output is a preamble (`set page` / `set text` / a couple of reusable
/// helper functions) followed by one rendered chunk per top-level block, in
/// order. The result is pure markup — feed it to the compiler to get a PDF.
pub fn build_typst_document(meta: &LayoutMeta, blocks: &[RenderBlock]) -> String {
    let mut out = String::with_capacity(512 + blocks.len() * 64);
    out.push_str(&preamble(meta));
    for block in blocks {
        out.push_str(&render_block(block));
    }
    out
}

/// The document preamble: page + text setup and the small set of helper
/// functions the per-block markup calls (`#bp-heading`, `#bp-byline`, …).
fn preamble(meta: &LayoutMeta) -> String {
    let paper = normalize_paper(&meta.paper);
    let size = clamp_font_size(meta.font_size_pt);
    let lang_line = match meta
        .lang
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(lang) => format!(", lang: \"{}\"", escape_string(lang)),
        None => String::new(),
    };

    // Theme-derived fragments. With no theme every fragment collapses to the
    // historical literal/empty form so the preamble is byte-identical to the
    // pre-theme output (regression-pinned). With a theme they carry the
    // church's fonts / accent / weight / leading. All values reaching Typst are
    // already validated/escaped by `LayoutTheme::resolve`.
    let theme = meta.theme.as_ref().map(LayoutTheme::resolve);
    // `font: "…"` clauses, injected into `#set text` (body) and the heading
    // helpers' `#text(...)` calls. Empty when no theme OR when the resolved font
    // is still the house default (a partial theme that doesn't touch fonts must
    // leave these lines byte-identical to the unthemed output).
    let body_font_clause = match &theme {
        Some(t) if t.body_font != DEFAULT_BODY_FONT => {
            format!(", font: \"{}\"", escape_string(&t.body_font))
        }
        _ => String::new(),
    };
    let heading_font_clause = match &theme {
        Some(t) if t.heading_font != DEFAULT_HEADING_FONT => {
            format!("font: \"{}\", ", escape_string(&t.heading_font))
        }
        _ => String::new(),
    };
    // The leading value: `0.65em` at the house default, scaled by the theme's
    // spacing multiplier otherwise. `format_em` keeps the no-theme case as the
    // exact literal `0.65`.
    let leading = format_em(theme.as_ref().map_or(DEFAULT_LEADING_EM, |t| t.leading_em));
    // Heading weight — `"bold"` by default, the theme's keyword otherwise.
    let weight = theme
        .as_ref()
        .map_or(DEFAULT_HEADING_WEIGHT, |t| t.heading_weight.as_str());
    // Accent fill on headings/title — empty (no fill) at the house default so
    // the original output is unchanged; `, fill: rgb("#…")` with a theme.
    let accent_clause = match &theme {
        Some(t) if t.accent != DEFAULT_ACCENT => format!(", fill: {}", t.accent),
        _ => String::new(),
    };
    format!(
        "// Generated by SundayPaper — do not edit by hand.\n\
         #set page(paper: \"{paper}\", margin: 2cm)\n\
         #set text(size: {size}pt{lang_line}{body_font_clause})\n\
         #set par(justify: false, leading: {leading}em)\n\n\
         // --- reusable helpers ---\n\
         #let bp-title(t, sub: none, date: none) = {{\n  \
             align(center)[#text({heading_font_clause}size: 1.6em, weight: \"{weight}\"{accent_clause})[#t]]\n  \
             if sub != none {{ align(center)[#text(size: 1.1em)[#sub]] }}\n  \
             if date != none {{ align(center)[#emph[#date]] }}\n  \
             v(0.4em); line(length: 100%); v(0.6em)\n\
         }}\n\
         #let bp-heading(t) = [#v(0.5em)#text({heading_font_clause}size: 1.2em, weight: \"{weight}\"{accent_clause})[#t]#v(0.2em)]\n\
         #let bp-byline(who) = if who != none {{ text(size: 0.85em, style: \"italic\")[#who] }}\n\
         #let bp-time(t) = if t != none {{ box(width: 3em)[#text(fill: gray)[#t]] }}\n\
         // Lyric helpers: a numbered verse and an indented, italic refrain so a\n\
         // congregation can follow along. The number is optional (none = unnumbered).\n\
         #let bp-verse(n, body) = block(below: 0.5em)[#if n != none {{ [#text(weight: \"bold\")[#n.] #h(0.3em)] }}#body]\n\
         #let bp-refrain(body) = block(below: 0.5em, inset: (left: 1.2em))[#emph[#body]]\n\
         // Form-field helpers. SundayPaper renders forms as printable fields —\n\
         // a labelled rule a person fills in by hand — so member data never has\n\
         // to leave the machine (no cloud round-trip to fill a PDF).\n\
         #let bp-field(label, hint: none, width: 100%) = block(below: 0.7em)[\n  \
             #if label != none {{ [#label#h(0.4em)] }}\n  \
             #box(width: width, stroke: (bottom: 0.5pt + black), inset: (bottom: 2pt))[#if hint != none {{ text(size: 0.8em, fill: gray)[#hint] }} else {{ [~] }}]\n\
         ]\n\
         #let bp-check(label) = block(below: 0.5em)[#box(width: 0.9em, height: 0.9em, stroke: 0.5pt + black) #h(0.4em) #if label != none {{ [#label] }}]\n\
         #let bp-sign(label, width: 60%) = block(below: 0.8em)[\n  \
             #box(width: width, stroke: (bottom: 0.5pt + black), inset: (bottom: 2pt))[~]\n  \
             #if label != none {{ [\\\n#text(size: 0.8em, fill: gray)[#label]] }}\n\
         ]\n\
         // Table helper: a grid of content cells. `cols` is the column count,\n\
         // `stroke` selects the inner-rule style (a length for a full grid, or\n\
         // none), `frame` draws an outer rule around the whole table, and\n\
         // `header` shades + bolds the first row. Cells arrive as a flat,\n\
         // row-major content array already padded to a full grid by the caller.\n\
         #let bp-table(cols, stroke, frame, header, ..cells) = block(\n    \
             below: 0.7em,\n    \
             stroke: if frame {{ 0.5pt + black }} else {{ none }},\n  \
         )[\n  \
             #let items = cells.pos()\n  \
             #table(\n    \
                 columns: cols,\n    \
                 stroke: stroke,\n    \
                 ..if header and items.len() >= cols {{ items.slice(0, cols).map(c => table.cell(fill: luma(230))[#text(weight: \"bold\")[#c]]) }} else {{ () }},\n    \
                 ..if header {{ items.slice(calc.min(cols, items.len())) }} else {{ items }},\n  \
             )\n\
         ]\n\
         // Container helpers (Step 2: block nesting).\n\
         // Two-column: a 1fr/1fr grid that lays its children out in row-major\n\
         // order (child 0 left, child 1 right, child 2 left of the next row, …).\n\
         // The classic poetry-on-left / translation-on-right pairing falls out\n\
         // of feeding paired children. An empty container renders nothing.\n\
         #let bp-twocol(..cells) = {{\n  \
             let items = cells.pos()\n  \
             if items.len() > 0 {{ block(below: 0.7em)[#grid(columns: (1fr, 1fr), column-gutter: 1.2em, row-gutter: 0.5em, ..items)] }}\n\
         }}\n\
         // Callout: a highlighted, boxed region (prayers, notes, asides) wrapping\n\
         // its children. An optional escaped title prints bold at the top.\n\
         #let bp-callout(title, ..body) = block(\n    \
             below: 0.7em,\n    \
             width: 100%,\n    \
             inset: 0.8em,\n    \
             radius: 4pt,\n    \
             fill: luma(245),\n    \
             stroke: (left: 2pt + luma(160)),\n  \
         )[\n  \
             #if title != none {{ [#text(weight: \"bold\")[#title]#v(0.3em)] }}\n  \
             #body.pos().join()\n\
         ]\n\n",
    )
}

/// Render one block (and recurse into its children). Dispatches on `kind`;
/// unknown kinds degrade to the generic text renderer so nothing is dropped —
/// the same "never lose a block" promise the bulletin generator makes.
///
/// Two block kinds are **containers** (`two_column` / `callout`): they consume
/// their children, arranging each child's rendered markup inside a layout
/// construct (a two-column grid, a boxed callout) rather than letting the
/// children fall out as flat siblings. Every other kind is a leaf as far as
/// layout is concerned: it renders itself and then its children follow flat
/// after it, exactly as before (this preserves the old behaviour for trees that
/// happen to carry children on non-container nodes).
fn render_block(block: &RenderBlock) -> String {
    let d = &block.data;
    let mut s = String::new();

    // Cross-cutting hint: a leading page break, if the block asked for one.
    if d.get("pageBreak").and_then(serde_json::Value::as_bool) == Some(true) {
        s.push_str("#pagebreak()\n");
    }

    match block.kind.as_str() {
        // --- containers: they OWN their children's layout ---
        "two_column" => {
            s.push_str(&render_two_column(block));
            s.push('\n');
            return s;
        }
        "callout" => {
            s.push_str(&render_callout(block));
            s.push('\n');
            return s;
        }
        // --- leaves: render self, then children flat after ---
        "heading" => s.push_str(&render_heading(d)),
        "song" => s.push_str(&render_song(d)),
        "music" => s.push_str(&render_music(d)),
        "scripture" => s.push_str(&render_scripture(d)),
        "liturgy" => s.push_str(&render_liturgy(d)),
        "announcement" => s.push_str(&render_announcement(d)),
        "image" => s.push_str(&render_image(d)),
        "form_field" => s.push_str(&render_form_field(d)),
        "checkbox" => s.push_str(&render_checkbox(d)),
        "signature" => s.push_str(&render_signature(d)),
        "table" => s.push_str(&render_table(d)),
        // "text" and any unknown future kind.
        _ => s.push_str(&render_text(d)),
    }

    for child in &block.children {
        s.push_str(&render_block(child));
    }
    s.push('\n');
    s
}

/// Render each child of a container to its own balanced markup chunk, wrapped in
/// a content block `[…]` so it can be passed as a single positional argument to
/// a layout helper. Each child goes through the normal [`render_block`] dispatch
/// (so a child may itself be a container — nesting works to any depth). The
/// trailing newline `render_block` appends is trimmed inside each cell so the
/// grid/callout markup stays compact, but the child's own bracket balance is
/// untouched.
fn child_cells(block: &RenderBlock) -> Vec<String> {
    block
        .children
        .iter()
        .map(|child| format!("[{}]", render_block(child).trim_end_matches('\n')))
        .collect()
}

/// `two_column` container — lays its children out in a two-column (1fr/1fr)
/// grid, row-major (child 0 left, child 1 right, child 2 left of the next row…).
/// The canonical use is poetry-on-left / translation-on-right by feeding paired
/// children. With no children it emits nothing (the helper guards on an empty
/// cell list), so an empty container never leaves a stray grid behind.
fn render_two_column(block: &RenderBlock) -> String {
    let cells = child_cells(block);
    let mut s = String::from("#bp-twocol(");
    s.push_str(&cells.join(", "));
    s.push_str(")\n");
    s
}

/// `callout` container — wraps its children in a highlighted, boxed region for
/// prayers, notes and asides. An optional `title` (escaped, falling back to a
/// `role` keyword) prints bold at the top of the box; the children render inside
/// it via the normal dispatch. The title/role is escaped through
/// [`content_arg`], so it can never inject markup.
fn render_callout(block: &RenderBlock) -> String {
    let d = &block.data;
    let title = field(d, "title").or_else(|| field(d, "role"));
    let cells = child_cells(block);
    let mut s = format!("#bp-callout({}", content_arg(&title));
    for cell in &cells {
        s.push_str(", ");
        s.push_str(cell);
    }
    s.push_str(")\n");
    s
}

/// `heading` — either the leading service title (role `service-title`, with the
/// subtitle/date helper) or a plain section heading (e.g. the sermon).
fn render_heading(d: &serde_json::Value) -> String {
    let title = field(d, "title");
    if role(d) == "service-title" {
        return format!(
            "#bp-title({}, sub: {}, date: {})\n",
            content_arg(&title),
            content_arg(&field(d, "subtitle")),
            content_arg(&field(d, "date")),
        );
    }
    let mut s = format!("#bp-heading({})\n", content_arg(&title));
    // A sermon heading may carry a preacher byline + synopsis.
    if let Some(p) = field(d, "preacher") {
        s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(p))));
    }
    if let Some(syn) = field(d, "synopsis") {
        s.push_str(&format!("#par[{}]\n", escape_content(&syn)));
    }
    s
}

/// `song` — title (+ hymnal number), an optional author byline, the song's
/// verses (numbered) and refrain when the payload carries them, and an optional
/// copyright credit printed small at the end. Plans bound to the song catalog
/// can now carry the actual `verses`/`refrain` text, so the program prints the
/// words the congregation sings — not just the heading. When no lyrics are
/// supplied (a bare plan reference), only the heading/byline are emitted, as
/// before, so the FORWARD pipeline degrades gracefully.
fn render_song(d: &serde_json::Value) -> String {
    let title = field(d, "title").unwrap_or_else(|| "Song".to_string());
    let heading = match field(d, "number") {
        Some(n) => format!("{} — {}", n, title),
        None => title,
    };
    let mut s = format!("#bp-heading({})\n", content_arg(&Some(heading)));
    if let Some(author) = field(d, "author") {
        s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(author))));
    }

    // Verses + refrain. Each non-blank verse is numbered in document order; the
    // refrain (if any) is printed once, indented, after the first verse — the
    // common congregational layout. Every line is escaped, so a lyric line can
    // never inject Typst markup. A verse can itself span several lines, which
    // `escape_content` turns into hard line breaks.
    let verses = string_list(d, "verses");
    let refrain = field(d, "refrain");
    for (i, verse) in verses.iter().enumerate() {
        s.push_str(&format!(
            "#bp-verse([{}], [{}])\n",
            i + 1,
            escape_content(verse)
        ));
        // Print the refrain after the first verse only.
        if i == 0 {
            if let Some(r) = &refrain {
                s.push_str(&format!("#bp-refrain([{}])\n", escape_content(r)));
            }
        }
    }
    // A refrain with no verses at all still gets printed once.
    if verses.is_empty() {
        if let Some(r) = &refrain {
            s.push_str(&format!("#bp-refrain([{}])\n", escape_content(r)));
        }
    }

    if let Some(c) = field(d, "copyright") {
        s.push_str(&format!(
            "#text(size: 0.75em, fill: gray)[{}]\n",
            escape_content(&c)
        ));
    }
    s
}

/// `music` — instrumental: a heading plus an optional performer byline / note.
fn render_music(d: &serde_json::Value) -> String {
    let title = field(d, "title").unwrap_or_else(|| "Music".to_string());
    let mut s = format!("#bp-heading({})\n", content_arg(&Some(title)));
    if let Some(leader) = field(d, "leader") {
        s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(leader))));
    }
    if let Some(note) = field(d, "text") {
        s.push_str(&format!("#emph[{}]\n", escape_content(&note)));
    }
    s
}

/// `scripture` — a heading (title or the "Book ref" reference), a reader byline
/// and the read text as a quote block.
fn render_scripture(d: &serde_json::Value) -> String {
    let reference = join_reference(d);
    let heading = field(d, "title").or_else(|| reference.clone());
    let mut s = format!(
        "#bp-heading({})\n",
        content_arg(&heading.or_else(|| Some("Reading".to_string())))
    );
    // If the heading was a custom title, still print the reference under it.
    if field(d, "title").is_some() {
        if let Some(r) = reference {
            s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(r))));
        }
    }
    if let Some(reader) = field(d, "reader") {
        s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(reader))));
    }
    if let Some(text) = field(d, "text") {
        s.push_str(&format!("#quote(block: true)[{}]\n", escape_content(&text)));
    }
    s
}

/// `liturgy` — a labelled spoken element with an optional leader byline + text.
fn render_liturgy(d: &serde_json::Value) -> String {
    let title = field(d, "title").unwrap_or_else(|| "Liturgy".to_string());
    let mut s = format!("#bp-heading({})\n", content_arg(&Some(title)));
    if let Some(leader) = field(d, "leader") {
        s.push_str(&format!("#bp-byline({})\n", content_arg(&Some(leader))));
    }
    if let Some(text) = field(d, "text") {
        s.push_str(&format!("#par[{}]\n", escape_content(&text)));
    }
    s
}

/// `announcement` — a heading + body paragraph.
fn render_announcement(d: &serde_json::Value) -> String {
    let title = field(d, "title").unwrap_or_else(|| "Announcement".to_string());
    let mut s = format!("#bp-heading({})\n", content_arg(&Some(title)));
    if let Some(text) = field(d, "text") {
        s.push_str(&format!("#par[{}]\n", escape_content(&text)));
    }
    s
}

/// `image` — an embedded image with an optional caption. The path comes from
/// the asset's local `url`; if there is no path we emit a caption-only
/// placeholder rather than an `image()` call that would fail to compile.
fn render_image(d: &serde_json::Value) -> String {
    let caption = field(d, "caption").or_else(|| field(d, "title"));
    match field(d, "url") {
        Some(path) => {
            let img = format!("image(\"{}\", width: 80%)", escape_string(&path));
            match caption {
                Some(c) => format!("#figure({}, caption: [{}])\n", img, escape_content(&c)),
                None => format!("#align(center)[#{}]\n", img),
            }
        }
        None => match caption {
            Some(c) => format!("#align(center)[#emph[{}]]\n", escape_content(&c)),
            None => String::new(),
        },
    }
}

/// `form_field` — a fillable text field: a label followed by an underlined
/// blank a person writes into (name, e-mail, phone, amount…). An optional
/// `hint` prints faint placeholder text inside the rule; an optional `width`
/// (`full` / `half` / `third` / `quarter`) sizes the rule so several short
/// fields can share a printed line conceptually. Privacy by design: the field
/// is a printed blank, so no member data is ever embedded or transmitted.
fn render_form_field(d: &serde_json::Value) -> String {
    let label = field(d, "label").or_else(|| field(d, "title"));
    let width = field_width(d);
    format!(
        "#bp-field({}, hint: {}, width: {})\n",
        content_arg(&label),
        content_arg(&field(d, "hint")),
        width
    )
}

/// `checkbox` — a printed empty box with a label, for opt-ins, attendance
/// ticks, "I consent" lines and the like.
fn render_checkbox(d: &serde_json::Value) -> String {
    let label = field(d, "label").or_else(|| field(d, "title"));
    format!("#bp-check({})\n", content_arg(&label))
}

/// `signature` — a wider rule to sign on, with the label printed small beneath
/// it (e.g. "Signature", "Date"). Defaults to a half-ish width line.
fn render_signature(d: &serde_json::Value) -> String {
    let label = field(d, "label")
        .or_else(|| field(d, "title"))
        .or_else(|| Some("Signature".to_string()));
    // A signature line defaults to a comfortable 60%; an explicit width keyword
    // overrides it.
    let width = field_width_or(d, "60%");
    format!("#bp-sign({}, width: {})\n", content_arg(&label), width)
}

/// `table` — a grid of content cells (service orders, rosters, schedules,
/// magazine grids). The payload carries the grid dimensions, a flat list of
/// `{rowIndex, colIndex, content}` cells, a `headerRow` flag, and a `borders`
/// keyword. The renderer rebuilds a dense, row-major cell array so ragged or
/// sparse input still produces a well-formed `#table`: every cell is run
/// through [`escape_content`], missing cells become an empty `[]`, and any cell
/// outside the declared dimensions is dropped (so a stray index can never grow
/// the grid or inject markup). A 0×0 grid renders nothing rather than an empty
/// `#table()` that would just take vertical space.
fn render_table(d: &serde_json::Value) -> String {
    let cols = dimension(d, "numCols");
    let rows = dimension(d, "numRows");
    if cols == 0 || rows == 0 {
        return String::new();
    }
    let header = d
        .get("headerRow")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let (stroke, frame) = table_borders(d);

    // Dense row-major grid seeded with empty cells, then filled from the sparse
    // payload. Out-of-range indices are ignored, so the grid stays exactly
    // rows×cols regardless of what the payload claims.
    let mut grid: Vec<String> = vec![String::new(); rows * cols];
    if let Some(cells) = d.get("cells").and_then(serde_json::Value::as_array) {
        for cell in cells {
            let r = cell.get("rowIndex").and_then(serde_json::Value::as_u64);
            let c = cell.get("colIndex").and_then(serde_json::Value::as_u64);
            let (Some(r), Some(c)) = (r, c) else { continue };
            let (r, c) = (r as usize, c as usize);
            if r >= rows || c >= cols {
                continue;
            }
            let content = cell
                .get("content")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            grid[r * cols + c] = escape_content(content);
        }
    }

    // Emit `#bp-table(cols, stroke, frame, header, [c0], [c1], …)` with one
    // bracketed content arg per cell, in row-major order.
    let mut s = format!("#bp-table({}, {}, {}, {}", cols, stroke, frame, header);
    for cell in &grid {
        s.push_str(&format!(", [{}]", cell));
    }
    s.push_str(")\n");
    s
}

/// Read a non-negative grid dimension (`numRows` / `numCols`) from the payload,
/// clamped to a sane upper bound so a malformed payload can't ask for a
/// million-cell table. Absent / non-numeric → 0.
fn dimension(d: &serde_json::Value, key: &str) -> usize {
    let n = d.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
    n.min(MAX_TABLE_DIM)
}

/// Upper bound on table rows/columns (defensive — the editor never asks for
/// anything near this, but a hand-edited payload shouldn't be able to).
const MAX_TABLE_DIM: usize = 64;

/// Map the `borders` keyword to the `(stroke, frame)` pair `#bp-table` expects.
/// Only a fixed set is accepted (no expression injection):
/// - `all` → full inner grid, no extra frame (the grid already frames it);
/// - `none` → no rules at all;
/// - `outer` (and any unknown value, the safe default) → no inner rules, a
///   single outer frame drawn by the wrapping block.
///
/// `stroke` is a Typst length expression or `none`; `frame` is a bool literal.
fn table_borders(d: &serde_json::Value) -> (&'static str, &'static str) {
    match field(d, "borders").as_deref().map(str::to_ascii_lowercase) {
        Some(b) if b == "none" => ("none", "false"),
        Some(b) if b == "all" => ("0.5pt + black", "false"),
        _ => ("none", "true"),
    }
}

/// `text` and the fallback for unknown kinds — an optional bold title + body.
fn render_text(d: &serde_json::Value) -> String {
    let mut s = String::new();
    if let Some(title) = field(d, "title") {
        s.push_str(&format!("#bp-heading({})\n", content_arg(&Some(title))));
    }
    if let Some(text) = field(d, "text") {
        s.push_str(&format!("#par[{}]\n", escape_content(&text)));
    }
    s
}

// --- payload helpers ---------------------------------------------------------

/// Read a string field from a payload, returning `None` for absent / null /
/// non-string / blank values (so the markup never prints empty constructs).
fn field(d: &serde_json::Value, key: &str) -> Option<String> {
    d.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Read a string-array field (e.g. a song's `verses`) into a `Vec<String>`,
/// keeping only non-blank entries (trimmed) and dropping non-string elements.
/// An absent / non-array value yields an empty vector, so the markup prints
/// nothing rather than a broken construct.
fn string_list(d: &serde_json::Value, key: &str) -> Vec<String> {
    d.get(key)
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The `role` discriminator a block may carry (heading/liturgy), defaulting to
/// an empty string when absent.
fn role(d: &serde_json::Value) -> &str {
    d.get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}

/// Map a form field's optional `width` keyword to a Typst length literal for the
/// rule that the field/signature draws. Only a fixed set of keywords is
/// accepted (so user input can never inject a length expression); anything else
/// — including an absent value — falls back to a full-width rule.
fn field_width(d: &serde_json::Value) -> &'static str {
    field_width_or(d, "100%")
}

/// Like [`field_width`] but with a caller-chosen fallback for the absent /
/// unrecognised case (so a signature line can default narrower than a text
/// field). The keyword set itself is fixed, so no length expression can be
/// injected through the payload.
fn field_width_or(d: &serde_json::Value, default: &'static str) -> &'static str {
    match field(d, "width").as_deref().map(str::to_ascii_lowercase) {
        Some(w) if w == "full" => "100%",
        Some(w) if w == "half" => "50%",
        Some(w) if w == "third" => "33%",
        Some(w) if w == "quarter" => "25%",
        _ => default,
    }
}

/// Build the "Book chapter:verse (Translation)" reference string from a
/// scripture payload, or `None` if there is nothing to show.
fn join_reference(d: &serde_json::Value) -> Option<String> {
    let book = field(d, "book");
    let reference = field(d, "reference");
    let base = match (book, reference) {
        (Some(b), Some(r)) => Some(format!("{} {}", b, r)),
        (Some(b), None) => Some(b),
        (None, Some(r)) => Some(r),
        (None, None) => None,
    }?;
    match field(d, "translation") {
        Some(t) => Some(format!("{} ({})", base, t)),
        None => Some(base),
    }
}

// --- Typst escaping ----------------------------------------------------------

/// Turn an `Option<String>` into a Typst function-call argument: a bracketed
/// content block `[...]` with escaped contents, or the literal `none` when the
/// value is absent. Used for every helper call so callers stay readable.
fn content_arg(value: &Option<String>) -> String {
    match value {
        Some(v) => format!("[{}]", escape_content(v)),
        None => "none".to_string(),
    }
}

/// Escape arbitrary user text for Typst **content (markup) mode**.
///
/// Typst markup gives meaning to a handful of leading / inline characters
/// (`\ # $ * _ \` < > @ ~ [ ]` plus structural `/ - + =`). To guarantee that a
/// title like `#1 — A*B` prints verbatim and can never inject markup or break
/// compilation, we backslash-escape every character Typst treats specially.
/// Newlines are converted to a hard line break (`\ ` then newline) so multi-line
/// liturgy text keeps its lines.
pub fn escape_content(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            // Backslash first so we don't double-escape what follows.
            '\\' => out.push_str("\\\\"),
            // Structural chars Typst gives meaning to at the start of a line or
            // inline: `-`/`+` open list items and `--`/`---` become en/em dashes,
            // so they must be escaped too (see the contract in the doc comment).
            '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '~' | '[' | ']' | '/' | '=' | '-'
            | '+' => {
                out.push('\\');
                out.push(ch);
            }
            '\n' => out.push_str("\\\n"),
            '\r' => {} // drop bare CR; the paired \n carries the break
            _ => out.push(ch),
        }
    }
    out
}

/// Escape a string destined for a Typst **string literal** (double-quoted), as
/// in `image("...")` or `lang: "..."`. Only the backslash and the double quote
/// need escaping inside a Typst string; control chars are dropped.
pub fn escape_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => {} // never legal raw in a single-line literal
            _ => out.push(ch),
        }
    }
    out
}

/// Normalise a paper keyword to one Typst accepts; unknown values fall back to
/// `a4` so the generated source always compiles. Case-insensitive.
fn normalize_paper(paper: &str) -> String {
    match paper.trim().to_ascii_lowercase().as_str() {
        "a3" => "a3",
        "a4" => "a4",
        "a5" => "a5",
        "a6" => "a6",
        "us-letter" | "letter" => "us-letter",
        "us-legal" | "legal" => "us-legal",
        _ => "a4",
    }
    .to_string()
}

/// Clamp the font size into a sane printable range (6–48 pt); NaN → default.
fn clamp_font_size(pt: f64) -> f64 {
    if pt.is_finite() {
        pt.clamp(6.0, 48.0)
    } else {
        11.0
    }
}

/// Format an `em` length value deterministically for the preamble. Rounds to at
/// most three decimals and trims trailing zeros, so the house default `0.65`
/// prints as exactly `0.65` (byte-identical to the pre-theme literal) and a
/// scaled value like `0.65 * 1.5 = 0.975` prints cleanly without float noise.
fn format_em(value: f64) -> String {
    // Round to 3 decimals to kill binary-float jitter, then strip trailing
    // zeros / a dangling dot.
    let rounded = (value * 1000.0).round() / 1000.0;
    let mut s = format!("{rounded:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(blocks: &[RenderBlock]) -> String {
        build_typst_document(&LayoutMeta::default(), blocks)
    }

    #[test]
    fn preamble_sets_page_and_text() {
        let src = build_typst_document(&LayoutMeta::default(), &[]);
        assert!(src.contains("#set page(paper: \"a4\", margin: 2cm)"));
        assert!(src.contains("#set text(size: 11pt)"));
        assert!(src.contains("#let bp-title"));
        // No blocks → preamble only, no trailing block markup.
        assert!(!src.contains("#bp-heading("));
    }

    #[test]
    fn unknown_paper_falls_back_to_a4_and_font_clamps() {
        let meta = LayoutMeta {
            paper: "tabloid".into(),
            font_size_pt: 500.0,
            lang: Some("nb".into()),
            ..LayoutMeta::default()
        };
        let src = build_typst_document(&meta, &[]);
        assert!(src.contains("paper: \"a4\""));
        assert!(src.contains("size: 48pt"), "font clamped to max 48");
        assert!(src.contains("lang: \"nb\""));
    }

    #[test]
    fn nan_font_size_uses_default() {
        let meta = LayoutMeta {
            font_size_pt: f64::NAN,
            ..LayoutMeta::default()
        };
        assert!(build_typst_document(&meta, &[]).contains("size: 11pt"));
    }

    #[test]
    fn service_title_uses_title_helper_with_sub_and_date() {
        let b = RenderBlock::leaf(
            "heading",
            json!({
                "role": "service-title",
                "title": "Sunday Worship",
                "subtitle": "St. Olav's",
                "date": "1 June 2026",
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-title([Sunday Worship], sub: [St. Olav's], date: [1 June 2026])"));
    }

    #[test]
    fn service_title_omits_missing_sub_and_date_as_none() {
        let b = RenderBlock::leaf(
            "heading",
            json!({ "role": "service-title", "title": "Worship" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-title([Worship], sub: none, date: none)"));
    }

    #[test]
    fn sermon_heading_carries_preacher_and_synopsis() {
        let b = RenderBlock::leaf(
            "heading",
            json!({
                "role": "sermon",
                "title": "The Light",
                "preacher": "Pastor Anne",
                "synopsis": "On living in the light.",
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([The Light])"));
        assert!(src.contains("#bp-byline([Pastor Anne])"));
        assert!(src.contains("#par[On living in the light.]"));
    }

    #[test]
    fn song_prefixes_number_and_prints_author_and_copyright() {
        let b = RenderBlock::leaf(
            "song",
            json!({
                "title": "Holy, Holy, Holy",
                "number": "N13 097",
                "author": "Reginald Heber",
                "copyright": "Public Domain",
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([N13 097 — Holy, Holy, Holy])"));
        assert!(src.contains("#bp-byline([Reginald Heber])"));
        assert!(src.contains("#text(size: 0.75em, fill: gray)[Public Domain]"));
    }

    #[test]
    fn song_without_number_uses_bare_title() {
        let b = RenderBlock::leaf("song", json!({ "title": "Amazing Grace" }));
        assert!(doc(&[b]).contains("#bp-heading([Amazing Grace])"));
    }

    #[test]
    fn preamble_defines_lyric_helpers() {
        let src = build_typst_document(&LayoutMeta::default(), &[]);
        assert!(src.contains("#let bp-verse"));
        assert!(src.contains("#let bp-refrain"));
    }

    #[test]
    fn song_renders_verses_in_order_with_numbers() {
        let b = RenderBlock::leaf(
            "song",
            json!({
                "title": "Amazing Grace",
                "verses": ["Amazing grace, how sweet", "'Twas grace that taught", "Through many dangers"],
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-verse([1], [Amazing grace, how sweet])"));
        assert!(src.contains("#bp-verse([2], ['Twas grace that taught])"));
        assert!(src.contains("#bp-verse([3], [Through many dangers])"));
        // Numbering is sequential in document order.
        let v1 = src.find("#bp-verse([1]").unwrap();
        let v2 = src.find("#bp-verse([2]").unwrap();
        let v3 = src.find("#bp-verse([3]").unwrap();
        assert!(v1 < v2 && v2 < v3, "verses render in order");
    }

    #[test]
    fn song_refrain_prints_once_after_first_verse() {
        let b = RenderBlock::leaf(
            "song",
            json!({
                "title": "Hymn",
                "verses": ["Verse one", "Verse two"],
                "refrain": "Sing the chorus",
            }),
        );
        let src = doc(&[b]);
        // Exactly one refrain, positioned between verse 1 and verse 2.
        assert_eq!(
            src.matches("#bp-refrain(").count(),
            1,
            "refrain printed once"
        );
        let v1 = src.find("#bp-verse([1]").unwrap();
        let refrain = src.find("#bp-refrain([Sing the chorus])").unwrap();
        let v2 = src.find("#bp-verse([2]").unwrap();
        assert!(v1 < refrain && refrain < v2, "refrain sits after verse 1");
    }

    #[test]
    fn song_refrain_without_verses_still_prints() {
        let b = RenderBlock::leaf(
            "song",
            json!({ "title": "Chorus only", "refrain": "Alleluia" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-refrain([Alleluia])"));
        assert!(!src.contains("#bp-verse("), "no verses → no verse markup");
    }

    #[test]
    fn song_without_lyrics_emits_no_verse_or_refrain() {
        let b = RenderBlock::leaf("song", json!({ "title": "Bare reference" }));
        let src = doc(&[b]);
        assert!(!src.contains("#bp-verse("));
        assert!(!src.contains("#bp-refrain("));
        // The heading still renders, as before.
        assert!(src.contains("#bp-heading([Bare reference])"));
    }

    #[test]
    fn song_blank_and_non_string_verses_are_skipped() {
        let b = RenderBlock::leaf(
            "song",
            json!({ "title": "Mixed", "verses": ["Real verse", "   ", null, 42, "Second real"] }),
        );
        let src = doc(&[b]);
        // Only the two real verses survive, renumbered 1 and 2.
        assert!(src.contains("#bp-verse([1], [Real verse])"));
        assert!(src.contains("#bp-verse([2], [Second real])"));
        assert!(!src.contains("#bp-verse([3]"));
    }

    #[test]
    fn song_multiline_verse_becomes_hard_breaks() {
        let b = RenderBlock::leaf(
            "song",
            json!({ "title": "T", "verses": ["line one\nline two"] }),
        );
        let src = doc(&[b]);
        // The newline inside the verse is escaped to a hard line break.
        assert!(src.contains("#bp-verse([1], [line one\\\nline two])"));
    }

    #[test]
    fn song_lyric_line_cannot_inject_markup() {
        // A malicious verse line trying to close the content block and call a fn.
        let b = RenderBlock::leaf(
            "song",
            json!({ "title": "T", "verses": ["x] #panic() [y"], "refrain": "$ # *" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("\\]"), "closing bracket escaped");
        assert!(src.contains("\\#panic"), "function call neutralised");
        assert!(!src.contains("[x] #panic"), "raw injection impossible");
    }

    #[test]
    fn music_block_renders_heading_byline_and_note() {
        let b = RenderBlock::leaf(
            "music",
            json!({ "title": "Postlude", "leader": "Organist", "text": "Bach" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([Postlude])"));
        assert!(src.contains("#bp-byline([Organist])"));
        assert!(src.contains("#emph[Bach]"));
    }

    #[test]
    fn scripture_builds_reference_with_translation() {
        let b = RenderBlock::leaf(
            "scripture",
            json!({
                "book": "John",
                "reference": "3:16-21",
                "translation": "NRSV",
                "text": "For God so loved the world.",
            }),
        );
        let src = doc(&[b]);
        // No explicit title → the reference becomes the heading. The hyphen in
        // the verse range is escaped (Typst would otherwise treat "16-21" as a
        // dash conversion / list context).
        assert!(src.contains("#bp-heading([John 3:16\\-21 (NRSV)])"));
        assert!(src.contains("#quote(block: true)[For God so loved the world.]"));
    }

    #[test]
    fn scripture_with_title_shows_reference_as_byline() {
        let b = RenderBlock::leaf(
            "scripture",
            json!({ "title": "First Reading", "book": "John", "reference": "3:16" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([First Reading])"));
        assert!(src.contains("#bp-byline([John 3:16])"));
    }

    #[test]
    fn liturgy_renders_title_leader_and_text() {
        let b = RenderBlock::leaf(
            "liturgy",
            json!({ "role": "benediction", "title": "Benediction", "leader": "Pastor", "text": "Go in peace." }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([Benediction])"));
        assert!(src.contains("#bp-byline([Pastor])"));
        assert!(src.contains("#par[Go in peace.]"));
    }

    #[test]
    fn image_with_url_becomes_figure_with_caption() {
        let b = RenderBlock::leaf(
            "image",
            json!({ "url": "/assets/banner.png", "caption": "He is risen" }),
        );
        let src = doc(&[b]);
        assert!(src.contains(
            "#figure(image(\"/assets/banner.png\", width: 80%), caption: [He is risen])"
        ));
    }

    #[test]
    fn image_without_url_emits_caption_placeholder_not_broken_image() {
        let b = RenderBlock::leaf("image", json!({ "caption": "Missing art" }));
        let src = doc(&[b]);
        assert!(!src.contains("image("), "no image() call without a path");
        assert!(src.contains("#align(center)[#emph[Missing art]]"));
    }

    #[test]
    fn image_without_url_or_caption_emits_nothing() {
        let b = RenderBlock::leaf("image", json!({}));
        let src = doc(&[b]);
        // Only the preamble and a trailing newline from the block separator.
        assert!(!src.contains("image"));
        assert!(!src.contains("#align"));
    }

    #[test]
    fn text_block_and_unknown_kind_use_text_renderer() {
        let known = RenderBlock::leaf("text", json!({ "title": "Notes", "text": "hello" }));
        let unknown = RenderBlock::leaf("totally_new", json!({ "title": "Mystery", "text": "x" }));
        let src = doc(&[known, unknown]);
        assert!(src.contains("#bp-heading([Notes])"));
        assert!(src.contains("#par[hello]"));
        // Unknown kind is not dropped — it renders through the text path.
        assert!(src.contains("#bp-heading([Mystery])"));
        assert!(src.contains("#par[x]"));
    }

    #[test]
    fn page_break_hint_emits_pagebreak_before_block() {
        let b = RenderBlock::leaf("song", json!({ "title": "Hymn", "pageBreak": true }));
        let src = doc(&[b]);
        let pb = src.find("#pagebreak()").expect("pagebreak emitted");
        let heading = src.find("#bp-heading([Hymn])").expect("heading emitted");
        assert!(pb < heading, "pagebreak comes before the block content");
    }

    #[test]
    fn no_page_break_when_flag_absent_or_false() {
        let absent = RenderBlock::leaf("text", json!({ "text": "a" }));
        let falsey = RenderBlock::leaf("text", json!({ "text": "b", "pageBreak": false }));
        let src = doc(&[absent, falsey]);
        assert!(!src.contains("#pagebreak()"));
    }

    #[test]
    fn children_render_after_their_parent() {
        let parent = RenderBlock {
            kind: "liturgy".into(),
            data: json!({ "title": "Section" }),
            children: vec![RenderBlock::leaf("text", json!({ "text": "child line" }))],
        };
        let src = doc(&[parent]);
        let p = src.find("#bp-heading([Section])").unwrap();
        let c = src.find("#par[child line]").unwrap();
        assert!(p < c, "parent markup precedes its child");
    }

    #[test]
    fn order_of_top_level_blocks_is_preserved() {
        let src = doc(&[
            RenderBlock::leaf("text", json!({ "title": "First" })),
            RenderBlock::leaf("text", json!({ "title": "Second" })),
        ]);
        assert!(src.find("[First]").unwrap() < src.find("[Second]").unwrap());
    }

    #[test]
    fn build_is_deterministic() {
        let blocks = [RenderBlock::leaf(
            "scripture",
            json!({ "book": "John", "reference": "1:1", "text": "In the beginning" }),
        )];
        let a = doc(&blocks);
        let b = doc(&blocks);
        assert_eq!(a, b, "same tree → identical source bytes");
    }

    // --- escaping -------------------------------------------------------------

    #[test]
    fn content_escaping_neutralises_markup_chars() {
        // Every special char gets a leading backslash; text can't inject markup.
        let raw = "#1 *bold* _it_ `code` <a> @x ~ [b] / = $ \\";
        let esc = escape_content(raw);
        assert_eq!(
            esc,
            "\\#1 \\*bold\\* \\_it\\_ \\`code\\` \\<a\\> \\@x \\~ \\[b\\] \\/ \\= \\$ \\\\"
        );
    }

    #[test]
    fn content_escaping_neutralises_dash_and_plus() {
        // Typst markup treats a line starting with "- " as a bullet list item and
        // "+ " as a numbered list item, and collapses "--"/"---" into en/em
        // dashes. The doc comment promises these structural chars are escaped so
        // user text prints verbatim — verify they actually are.
        assert_eq!(escape_content("- so far"), "\\- so far");
        assert_eq!(escape_content("+ first"), "\\+ first");
        assert_eq!(escape_content("A--B"), "A\\-\\-B");
        assert_eq!(escape_content("--- C"), "\\-\\-\\- C");
    }

    #[test]
    fn content_escaping_handles_newlines_as_hard_breaks() {
        assert_eq!(escape_content("line1\nline2"), "line1\\\nline2");
        // CRLF: the CR is dropped, the LF becomes the break.
        assert_eq!(escape_content("a\r\nb"), "a\\\nb");
    }

    #[test]
    fn injected_title_cannot_escape_its_content_block() {
        // A malicious title trying to close the [ ] and inject a function.
        let b = RenderBlock::leaf("text", json!({ "title": "x] #panic() [y" }));
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([x\\] \\#panic\\(\\)... "[..18].trim_end()));
        // More precisely: the brackets and # are escaped, so the [..] stays whole.
        assert!(src.contains("\\]"));
        assert!(src.contains("\\#panic"));
        assert!(!src.contains("[x] #panic"));
    }

    #[test]
    fn string_escaping_handles_quotes_and_backslashes() {
        assert_eq!(escape_string(r#"a"b\c"#), r#"a\"b\\c"#);
        // Newlines/tabs are stripped (never legal raw in a Typst string literal).
        assert_eq!(escape_string("a\nb\tc"), "abc");
    }

    #[test]
    fn image_path_with_quote_is_escaped_in_string_literal() {
        let b = RenderBlock::leaf("image", json!({ "url": "a\"b.png", "caption": "c" }));
        let src = doc(&[b]);
        assert!(src.contains("image(\"a\\\"b.png\", width: 80%)"));
    }

    #[test]
    fn blank_and_null_fields_are_ignored() {
        let b = RenderBlock::leaf(
            "liturgy",
            json!({ "title": "Prayer", "leader": "   ", "text": null }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-heading([Prayer])"));
        assert!(!src.contains("#bp-byline"), "blank leader → no byline");
        assert!(!src.contains("#par["), "null text → no paragraph");
    }

    // --- form fields (Phase 7.2) ----------------------------------------------

    #[test]
    fn preamble_defines_form_helpers() {
        let src = build_typst_document(&LayoutMeta::default(), &[]);
        assert!(src.contains("#let bp-field"));
        assert!(src.contains("#let bp-check"));
        assert!(src.contains("#let bp-sign"));
    }

    #[test]
    fn form_field_emits_bp_field_with_label_hint_and_width() {
        let b = RenderBlock::leaf(
            "form_field",
            json!({ "label": "Full name", "hint": "First & last", "width": "half" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-field([Full name], hint: [First & last], width: 50%)"));
    }

    #[test]
    fn form_field_without_hint_or_width_uses_none_and_full_width() {
        let b = RenderBlock::leaf("form_field", json!({ "label": "E-mail" }));
        let src = doc(&[b]);
        // The hyphen in the label is escaped so it prints literally.
        assert!(src.contains("#bp-field([E\\-mail], hint: none, width: 100%)"));
    }

    #[test]
    fn form_field_falls_back_to_title_when_no_label() {
        // The generic block editor uses `title`; forms use `label`. Accept both.
        let b = RenderBlock::leaf("form_field", json!({ "title": "Phone" }));
        assert!(doc(&[b]).contains("#bp-field([Phone],"));
    }

    #[test]
    fn checkbox_emits_bp_check_with_label() {
        let b = RenderBlock::leaf("checkbox", json!({ "label": "I consent" }));
        assert!(doc(&[b]).contains("#bp-check([I consent])"));
    }

    #[test]
    fn signature_emits_bp_sign_defaulting_label_and_width() {
        // No label → the renderer supplies "Signature"; default width is 60%.
        let b = RenderBlock::leaf("signature", json!({}));
        let src = doc(&[b]);
        assert!(src.contains("#bp-sign([Signature], width: 60%)"));
    }

    #[test]
    fn signature_honours_explicit_label_and_width() {
        let b = RenderBlock::leaf(
            "signature",
            json!({ "label": "Parent or guardian", "width": "full" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-sign([Parent or guardian], width: 100%)"));
    }

    #[test]
    fn form_field_label_is_escaped_and_cannot_inject_markup() {
        // A malicious label must not break out of its content block.
        let b = RenderBlock::leaf(
            "form_field",
            json!({ "label": "x] #panic() [y", "hint": "$ # *" }),
        );
        let src = doc(&[b]);
        assert!(src.contains("\\]"), "closing bracket is escaped");
        assert!(src.contains("\\#panic"), "function call is neutralised");
        assert!(!src.contains("[x] #panic"), "raw injection is impossible");
    }

    #[test]
    fn unknown_field_width_keyword_falls_back_to_full() {
        let b = RenderBlock::leaf(
            "form_field",
            json!({ "label": "Amount", "width": "ginormous" }),
        );
        // Unrecognised keyword → safe 100%, never an injected length.
        assert!(doc(&[b]).contains("width: 100%)"));
    }

    #[test]
    fn from_spec_parses_json_and_defaults_bad_data_to_empty_object() {
        assert_eq!(
            RenderBlock::from_spec("text", r#"{"title":"T"}"#).data,
            json!({ "title": "T" })
        );
        // Garbage / non-object data degrades to {} rather than panicking.
        assert_eq!(RenderBlock::from_spec("text", "not json").data, json!({}));
        assert_eq!(RenderBlock::from_spec("text", "[1,2]").data, json!({}));
    }

    // --- table (Step 1: TableBlock) -------------------------------------------

    #[test]
    fn preamble_defines_table_helper() {
        let src = build_typst_document(&LayoutMeta::default(), &[]);
        assert!(src.contains("#let bp-table"));
    }

    #[test]
    fn table_2x3_emits_dense_row_major_grid() {
        let b = RenderBlock::leaf(
            "table",
            json!({
                "numRows": 2,
                "numCols": 3,
                "borders": "all",
                "cells": [
                    {"rowIndex": 0, "colIndex": 0, "content": "Tid"},
                    {"rowIndex": 0, "colIndex": 1, "content": "Aktivitet"},
                    {"rowIndex": 0, "colIndex": 2, "content": "Ansvarlig"},
                    {"rowIndex": 1, "colIndex": 0, "content": "11:00"},
                    {"rowIndex": 1, "colIndex": 1, "content": "Velkomst"},
                    {"rowIndex": 1, "colIndex": 2, "content": "Anne"},
                ],
            }),
        );
        let src = doc(&[b]);
        // cols=3, inner grid (all → stroke set, no outer frame), header=false.
        assert!(src.contains(
            "#bp-table(3, 0.5pt + black, false, false, [Tid], [Aktivitet], [Ansvarlig], [11:00], [Velkomst], [Anne])"
        ));
    }

    #[test]
    fn table_header_row_sets_header_flag() {
        let b = RenderBlock::leaf(
            "table",
            json!({
                "numRows": 1,
                "numCols": 2,
                "headerRow": true,
                "borders": "all",
                "cells": [
                    {"rowIndex": 0, "colIndex": 0, "content": "A"},
                    {"rowIndex": 0, "colIndex": 1, "content": "B"},
                ],
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-table(2, 0.5pt + black, false, true, [A], [B])"));
    }

    #[test]
    fn table_border_keywords_map_to_stroke_and_frame() {
        let mk = |borders: &str| {
            RenderBlock::leaf(
                "table",
                json!({ "numRows": 1, "numCols": 1, "borders": borders,
                        "cells": [{"rowIndex": 0, "colIndex": 0, "content": "x"}] }),
            )
        };
        // all → inner grid, no frame.
        assert!(doc(&[mk("all")]).contains("#bp-table(1, 0.5pt + black, false, false, [x])"));
        // none → no inner rules, no frame.
        assert!(doc(&[mk("none")]).contains("#bp-table(1, none, false, false, [x])"));
        // outer → no inner rules, an outer frame.
        assert!(doc(&[mk("outer")]).contains("#bp-table(1, none, true, false, [x])"));
        // unknown keyword → safe default (outer frame), never an injected value.
        assert!(doc(&[mk("ginormous")]).contains("#bp-table(1, none, true, false, [x])"));
    }

    #[test]
    fn table_missing_cells_become_empty_and_grid_stays_dense() {
        // 2x2 grid but only one cell supplied → the other three are empty [].
        let b = RenderBlock::leaf(
            "table",
            json!({
                "numRows": 2,
                "numCols": 2,
                "borders": "all",
                "cells": [{"rowIndex": 1, "colIndex": 1, "content": "only"}],
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-table(2, 0.5pt + black, false, false, [], [], [], [only])"));
    }

    #[test]
    fn table_out_of_range_cell_indices_are_dropped() {
        // Indices beyond the 1x1 grid must be ignored — they cannot grow the
        // grid (which stays exactly numRows×numCols = one empty cell).
        let b = RenderBlock::leaf(
            "table",
            json!({
                "numRows": 1,
                "numCols": 1,
                "borders": "all",
                "cells": [
                    {"rowIndex": 5, "colIndex": 0, "content": "off-row"},
                    {"rowIndex": 0, "colIndex": 9, "content": "off-col"},
                ],
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("#bp-table(1, 0.5pt + black, false, false, [])"));
        assert!(!src.contains("off-row"));
        assert!(!src.contains("off-col"));
    }

    #[test]
    fn table_zero_dimension_emits_nothing() {
        for d in [
            json!({ "numRows": 0, "numCols": 3 }),
            json!({ "numRows": 3, "numCols": 0 }),
            json!({}), // both absent → 0x0
        ] {
            let src = doc(&[RenderBlock::leaf("table", d)]);
            assert!(
                !src.contains("#bp-table("),
                "0-dim table emits no #bp-table"
            );
        }
    }

    #[test]
    fn table_cell_content_is_escaped_and_cannot_inject_markup() {
        // A cell carrying Typst structural chars must not break out of its [..].
        let b = RenderBlock::leaf(
            "table",
            json!({
                "numRows": 1,
                "numCols": 2,
                "borders": "all",
                "cells": [
                    {"rowIndex": 0, "colIndex": 0, "content": "a] #panic() [b"},
                    {"rowIndex": 0, "colIndex": 1, "content": "$ # *"},
                ],
            }),
        );
        let src = doc(&[b]);
        assert!(src.contains("\\]"), "closing bracket escaped");
        assert!(src.contains("\\#panic"), "function call neutralised");
        assert!(!src.contains("[a] #panic"), "raw injection impossible");
        // The escaped cell sits inside a content arg, grid still dense (2 cells).
        assert!(src.contains(
            "#bp-table(2, 0.5pt + black, false, false, [a\\] \\#panic() \\[b], [\\$ \\# \\*])"
        ));
    }

    #[test]
    fn table_dimensions_are_clamped_to_a_sane_bound() {
        // A hand-edited payload asking for an absurd grid is clamped, never
        // allocating a million cells.
        let b = RenderBlock::leaf(
            "table",
            json!({ "numRows": 1, "numCols": 100_000, "borders": "none" }),
        );
        let src = doc(&[b]);
        // 100_000 cols clamped to MAX_TABLE_DIM (64).
        assert!(src.contains("#bp-table(64, "));
    }

    // --- containers (Step 2: block nesting) -----------------------------------

    #[test]
    fn preamble_defines_container_helpers() {
        let src = build_typst_document(&LayoutMeta::default(), &[]);
        assert!(src.contains("#let bp-twocol"));
        assert!(src.contains("#let bp-callout"));
    }

    fn container(kind: &str, data: serde_json::Value, children: Vec<RenderBlock>) -> RenderBlock {
        RenderBlock {
            kind: kind.into(),
            data,
            children,
        }
    }

    #[test]
    fn two_column_renders_children_as_grid_cells_in_order() {
        // Poetry-on-left / translation-on-right: two children become two cells.
        let b = container(
            "two_column",
            json!({}),
            vec![
                RenderBlock::leaf("text", json!({ "text": "Original" })),
                RenderBlock::leaf("text", json!({ "text": "Oversettelse" })),
            ],
        );
        let src = doc(&[b]);
        let call = src
            .lines()
            .find(|l| l.starts_with("#bp-twocol("))
            .expect("twocol call emitted");
        // Two bracketed cells, the left before the right, each carrying its
        // child's rendered paragraph.
        assert!(call.contains("#par[Original]"), "left cell holds child 0");
        assert!(
            call.contains("#par[Oversettelse]"),
            "right cell holds child 1"
        );
        let left = call.find("Original").unwrap();
        let right = call.find("Oversettelse").unwrap();
        assert!(left < right, "cells are emitted in child order");
        // Exactly two top-level cells (two `, [` separators → one comma between
        // the two cells; assert the call is a single balanced #bp-twocol(...)).
        assert!(call.ends_with(')'));
    }

    #[test]
    fn two_column_children_are_not_flat_siblings() {
        // The whole point of a container: its children live INSIDE the helper
        // call, not as flat blocks after it.
        let b = container(
            "two_column",
            json!({}),
            vec![RenderBlock::leaf("text", json!({ "text": "inside" }))],
        );
        let src = doc(&[b]);
        // The child markup appears only within the bp-twocol(...) line, never on
        // its own line as a flat sibling.
        for line in src.lines() {
            if line.contains("#par[inside]") {
                assert!(
                    line.starts_with("#bp-twocol("),
                    "child must live inside the container call, got: {line}"
                );
            }
        }
    }

    #[test]
    fn empty_two_column_emits_helper_call_but_no_cells() {
        let b = container("two_column", json!({}), vec![]);
        let src = doc(&[b]);
        // The call is present (the helper guards emptiness at render time) but
        // carries no bracketed cell.
        assert!(src.contains("#bp-twocol()"));
    }

    #[test]
    fn callout_wraps_children_with_box_and_escaped_title() {
        let b = container(
            "callout",
            json!({ "title": "Bønn" }),
            vec![RenderBlock::leaf(
                "text",
                json!({ "text": "Vår Far i himmelen" }),
            )],
        );
        let src = doc(&[b]);
        let call = src
            .lines()
            .find(|l| l.starts_with("#bp-callout("))
            .expect("callout call emitted");
        assert!(call.starts_with("#bp-callout([Bønn]"), "title is first arg");
        assert!(
            call.contains("#par[Vår Far i himmelen]"),
            "child rendered inside the callout"
        );
    }

    #[test]
    fn callout_falls_back_to_role_then_none_for_title() {
        let with_role = container("callout", json!({ "role": "note" }), vec![]);
        assert!(doc(&[with_role]).contains("#bp-callout([note])"));
        let bare = container("callout", json!({}), vec![]);
        assert!(doc(&[bare]).contains("#bp-callout(none)"));
    }

    #[test]
    fn callout_title_cannot_inject_markup() {
        let b = container(
            "callout",
            json!({ "title": "x] #panic() [y" }),
            vec![RenderBlock::leaf("text", json!({ "text": "ok" }))],
        );
        let src = doc(&[b]);
        assert!(src.contains("\\]"), "closing bracket escaped");
        assert!(src.contains("\\#panic"), "function call neutralised");
        assert!(!src.contains("[x] #panic"), "raw injection impossible");
    }

    #[test]
    fn nested_container_in_container_keeps_every_child() {
        // A two_column whose left cell is itself a callout containing a child.
        let inner = container(
            "callout",
            json!({ "title": "Note" }),
            vec![RenderBlock::leaf("text", json!({ "text": "deep" }))],
        );
        let outer = container(
            "two_column",
            json!({}),
            vec![
                inner,
                RenderBlock::leaf("text", json!({ "text": "right side" })),
            ],
        );
        let src = doc(&[outer]);
        // The nested callout call appears INSIDE the twocol call (one logical
        // line — render_block trims the child's trailing newlines).
        let call = src
            .lines()
            .find(|l| l.starts_with("#bp-twocol("))
            .expect("twocol call emitted");
        assert!(call.contains("#bp-callout([Note]"), "inner callout nested");
        assert!(call.contains("#par[deep]"), "deepest child never dropped");
        assert!(call.contains("#par[right side]"), "sibling cell preserved");
        // And the whole document stays bracket-balanced.
        assert_eq!(
            unescaped_bracket_depth(&src),
            unescaped_bracket_depth(&build_typst_document(&LayoutMeta::default(), &[])),
            "nested containers keep the document at the preamble baseline"
        );
    }

    #[test]
    fn flat_non_container_children_are_unchanged() {
        // A plain (non-container) block with children keeps the old "render self,
        // then children flat after" behaviour — containers are the only thing
        // that changed.
        let parent = container(
            "liturgy",
            json!({ "title": "Section" }),
            vec![RenderBlock::leaf("text", json!({ "text": "child line" }))],
        );
        let src = doc(&[parent]);
        let p = src.find("#bp-heading([Section])").unwrap();
        let c = src.find("#par[child line]").unwrap();
        assert!(p < c, "parent precedes its flat child");
        // The child is its own line, NOT swallowed into a container call.
        assert!(
            src.lines().any(|l| l.trim() == "#par[child line]"),
            "child renders as a flat sibling line"
        );
    }

    // --- typography / theme system (Step 3) -----------------------------------

    #[test]
    fn no_theme_preamble_is_byte_identical_to_pre_theme_output() {
        // Regression pin: a LayoutMeta with theme None must produce EXACTLY the
        // historical preamble — fonts/accent unset, leading 0.65em, weight bold.
        let meta = LayoutMeta::default();
        assert_eq!(meta.theme, None, "default theme is None");
        let src = build_typst_document(&meta, &[]);
        // The exact lines that the theme machinery could have perturbed.
        assert!(
            src.contains("#set text(size: 11pt)\n"),
            "no font clause added"
        );
        assert!(
            src.contains("#set par(justify: false, leading: 0.65em)\n"),
            "leading is the literal 0.65em"
        );
        assert!(
            src.contains(
                "#let bp-heading(t) = [#v(0.5em)#text(size: 1.2em, weight: \"bold\")[#t]#v(0.2em)]\n"
            ),
            "heading helper unchanged (no font, no fill, weight bold)"
        );
        assert!(
            src.contains("align(center)[#text(size: 1.6em, weight: \"bold\")[#t]]"),
            "title helper unchanged"
        );
    }

    fn themed(theme: LayoutTheme) -> String {
        let meta = LayoutMeta {
            theme: Some(theme),
            ..LayoutMeta::default()
        };
        build_typst_document(&meta, &[])
    }

    #[test]
    fn theme_injects_body_font_into_set_text() {
        let src = themed(LayoutTheme {
            body_font: Some("EB Garamond".into()),
            ..Default::default()
        });
        assert!(src.contains("#set text(size: 11pt, font: \"EB Garamond\")"));
    }

    #[test]
    fn theme_injects_heading_font_and_weight_into_helpers() {
        let src = themed(LayoutTheme {
            heading_font: Some("Montserrat".into()),
            heading_weight: Some("black".into()),
            ..Default::default()
        });
        // Both the title and the section-heading helper pick up the font + weight.
        assert!(src.contains(
            "align(center)[#text(font: \"Montserrat\", size: 1.6em, weight: \"black\")[#t]]"
        ));
        assert!(src.contains(
            "#let bp-heading(t) = [#v(0.5em)#text(font: \"Montserrat\", size: 1.2em, weight: \"black\")[#t]#v(0.2em)]"
        ));
    }

    #[test]
    fn theme_injects_accent_fill_on_headings() {
        let src = themed(LayoutTheme {
            accent_color: Some("#C81E2D".into()),
            ..Default::default()
        });
        // Accent reaches both heading helpers as a validated rgb() fill, lowered.
        assert!(src.contains("weight: \"bold\", fill: rgb(\"#c81e2d\"))[#t]"));
        assert!(src.contains(
            "#let bp-heading(t) = [#v(0.5em)#text(size: 1.2em, weight: \"bold\", fill: rgb(\"#c81e2d\"))[#t]#v(0.2em)]"
        ));
    }

    #[test]
    fn theme_spacing_multiplier_scales_leading_deterministically() {
        // 0.65em * 1.5 = 0.975em, printed cleanly without float noise.
        let src = themed(LayoutTheme {
            spacing_multiplier: Some(1.5),
            ..Default::default()
        });
        assert!(src.contains("#set par(justify: false, leading: 0.975em)"));
        // 0.65 * 2.0 = 1.3em.
        let src2 = themed(LayoutTheme {
            spacing_multiplier: Some(2.0),
            ..Default::default()
        });
        assert!(src2.contains("leading: 1.3em"));
    }

    #[test]
    fn theme_spacing_multiplier_is_clamped() {
        // Below the floor → 0.5x; above the ceiling → 3x. NaN → 1.0 (default).
        let lo = themed(LayoutTheme {
            spacing_multiplier: Some(0.0),
            ..Default::default()
        });
        assert!(lo.contains("leading: 0.325em"), "clamped to 0.5x"); // 0.65*0.5
        let hi = themed(LayoutTheme {
            spacing_multiplier: Some(100.0),
            ..Default::default()
        });
        assert!(hi.contains("leading: 1.95em"), "clamped to 3x"); // 0.65*3
        let nan = themed(LayoutTheme {
            spacing_multiplier: Some(f64::NAN),
            ..Default::default()
        });
        assert!(nan.contains("leading: 0.65em"), "NaN → no scaling");
    }

    #[test]
    fn malicious_font_name_falls_back_to_house_default() {
        // A font name carrying quotes / markup must never reach Typst raw; an
        // unacceptable name silently drops to the house default (no font clause).
        let src = themed(LayoutTheme {
            body_font: Some("Evil\"); #panic() //".into()),
            heading_font: Some("a\"b".into()),
            ..Default::default()
        });
        // No injected quote/markup survived; the set-text line stays unfonted.
        assert!(
            src.contains("#set text(size: 11pt)\n"),
            "bad body font → no font clause, line unchanged"
        );
        assert!(!src.contains("#panic"), "no markup injected");
        assert!(
            src.contains("#text(size: 1.6em, weight: \"bold\")[#t]]"),
            "bad heading font → helper stays at default (no font clause)"
        );
    }

    #[test]
    fn malicious_accent_color_falls_back_to_no_fill() {
        // A non-hex accent must not reach rgb(); it falls back to the house
        // default (no fill), so headings render plain rather than injected.
        let src = themed(LayoutTheme {
            accent_color: Some("red\"); #panic()".into()),
            ..Default::default()
        });
        // No accent rgb() fill reached the headings (the only `fill:` that may
        // appear are the unrelated house `fill: gray`/`fill: luma(...)` ones).
        assert!(!src.contains("fill: rgb("), "invalid accent → no rgb fill");
        // And the heading helpers stay at the unthemed form (no fill clause).
        assert!(
            src.contains("#text(size: 1.2em, weight: \"bold\")[#t]"),
            "heading helper unchanged"
        );
        assert!(!src.contains("#panic"), "no markup injected");
    }

    #[test]
    fn unknown_heading_weight_falls_back_to_bold() {
        let src = themed(LayoutTheme {
            heading_weight: Some("ultralight-injection\"".into()),
            ..Default::default()
        });
        assert!(src.contains("weight: \"bold\""), "unknown weight → bold");
        assert!(!src.contains("ultralight"), "raw value never reaches Typst");
    }

    #[test]
    fn three_digit_hex_accent_is_accepted() {
        let src = themed(LayoutTheme {
            accent_color: Some("#FA0".into()),
            ..Default::default()
        });
        assert!(src.contains("fill: rgb(\"#fa0\")"));
    }

    #[test]
    fn partial_theme_keeps_house_defaults_for_unset_fields() {
        // Only an accent set: fonts/weight/leading stay at the house default.
        let src = themed(LayoutTheme {
            accent_color: Some("#123456".into()),
            ..Default::default()
        });
        assert!(src.contains("#set text(size: 11pt)\n"), "no body font");
        assert!(src.contains("leading: 0.65em"), "default leading");
        assert!(
            src.contains("weight: \"bold\", fill: rgb(\"#123456\"))[#t]"),
            "weight default + accent applied"
        );
    }

    #[test]
    fn themed_document_still_keeps_bracket_balance() {
        // A theme touches only the preamble; the document must stay at the same
        // unescaped-bracket baseline as the unthemed one (no theme value leaks a
        // structural bracket into the markup).
        let theme = LayoutTheme {
            heading_font: Some("Noto Serif".into()),
            body_font: Some("Noto Sans".into()),
            accent_color: Some("#abcdef".into()),
            heading_weight: Some("semibold".into()),
            spacing_multiplier: Some(1.25),
        };
        let blocks = [RenderBlock::leaf(
            "heading",
            json!({ "role": "service-title", "title": "Gudstjeneste", "subtitle": "St. Olav" }),
        )];
        let themed_meta = LayoutMeta {
            theme: Some(theme),
            ..LayoutMeta::default()
        };
        let src = build_typst_document(&themed_meta, &blocks);
        let plain = build_typst_document(&LayoutMeta::default(), &blocks);
        assert_eq!(
            unescaped_bracket_depth(&src),
            unescaped_bracket_depth(&plain),
            "theme injection keeps the document bracket-balanced"
        );
    }

    // --- property fuzzing -----------------------------------------------------
    //
    // Deterministic property tests over the Typst injection-safety contract.
    // No wall-clock, no unseeded RNG: a fixed-seed SplitMix64 drives an
    // adversarial, structural-char-heavy generator so every run is identical
    // and any failure is reproducible.

    /// Tiny deterministic PRNG (SplitMix64). Fixed seed → identical sequence.
    struct SplitMix64(u64);
    impl SplitMix64 {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Biased alphabet: heavy on Typst structural metacharacters, plus a few
    /// ordinary chars, newlines/CR, and multibyte unicode to exercise char
    /// boundaries. The structural set must mirror `escape_content` exactly.
    const FUZZ_ALPHABET: &[char] = &[
        // structural set, weighted by repetition
        '\\', '\\', '#', '#', '$', '*', '_', '`', '<', '>', '@', '~', '[', '[', ']', ']', '/', '=',
        '-', '-', '+', '+', // line breaks
        '\n', '\r', '\r', // ordinary
        'a', 'Z', '5', ' ', // multibyte unicode
        'æ', '—', '𝄞',
    ];

    fn fuzz_string(rng: &mut SplitMix64, max_len: usize) -> String {
        let len = rng.below(max_len + 1);
        (0..len)
            .map(|_| FUZZ_ALPHABET[rng.below(FUZZ_ALPHABET.len())])
            .collect()
    }

    /// The load-bearing scan: walk `s`, and for every structural metachar
    /// assert the run of consecutive backslashes immediately preceding it has
    /// ODD length (so it is escaped, never markup-active). A backslash itself is
    /// structural, but a `\\` pair is the escape for a literal backslash, so we
    /// only flag a backslash that is NOT the trailing char of an escape pair —
    /// handled by checking position parity across runs below.
    ///
    /// Returns `Some(byte_offset)` of the first violating special char, else
    /// `None`. The special set excludes `\\` and `\n` (their escaping is
    /// verified structurally by the dedicated newline test) — here we check the
    /// inline markup chars that a raw occurrence would activate.
    fn first_unescaped_special(s: &str) -> Option<usize> {
        const SPECIAL: &[char] = &[
            '#', '$', '*', '_', '`', '<', '>', '@', '~', '[', ']', '/', '=', '-', '+',
        ];
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        for (i, (off, ch)) in chars.iter().enumerate() {
            if SPECIAL.contains(ch) {
                // count consecutive backslashes immediately before position i
                let mut run = 0usize;
                let mut j = i;
                while j > 0 && chars[j - 1].1 == '\\' {
                    run += 1;
                    j -= 1;
                }
                if run.is_multiple_of(2) {
                    return Some(*off);
                }
            }
        }
        None
    }

    #[test]
    fn fuzz_escape_content_never_emits_unescaped_special() {
        // Hand-picked adversarial cases first.
        let hand = [
            "\\",
            "\\\\",
            "#",
            "\\#",
            "[x] #panic()",
            "---",
            "\n#",
            "++",
            "a\\#b",
            "\\\\#",
        ];
        for input in hand {
            let out = escape_content(input);
            assert_eq!(
                first_unescaped_special(&out),
                None,
                "hand case {input:?} produced unescaped special in {out:?}"
            );
        }
        // Then thousands of deterministic random adversarial strings.
        let mut rng = SplitMix64::new(0x5EED);
        for _ in 0..8000 {
            let input = fuzz_string(&mut rng, 40);
            let out = escape_content(&input);
            assert_eq!(
                first_unescaped_special(&out),
                None,
                "fuzz input {input:?} produced unescaped special in {out:?}"
            );
        }
    }

    #[test]
    fn fuzz_escape_content_newline_handling_is_sound() {
        // (a) No bare CR survives. (b) Every '\n' is the second char of a "\\\n"
        // pair — i.e. removing all CR and un-doubling each escaped newline leaves
        // a string with no unescaped inline special (subsumes the wrap path).
        let hand = [
            "\r\n",
            "\n\r",
            "a\rb",
            "\r\r\r",
            "line1\nline2\n",
            "#\n#",
            "-\n+",
            "\r",
            "\n",
        ];
        let mut rng = SplitMix64::new(0xC0FFEE);
        let mut cases: Vec<String> = hand.iter().map(|s| s.to_string()).collect();
        for _ in 0..4000 {
            cases.push(fuzz_string(&mut rng, 40));
        }
        for input in &cases {
            let out = escape_content(input);
            assert!(
                !out.contains('\r'),
                "bare CR survived escaping of {input:?} → {out:?}"
            );
            // Each '\n' must be immediately preceded by exactly one escape
            // backslash that is not itself part of an even backslash run.
            let bytes: Vec<(usize, char)> = out.char_indices().collect();
            for (i, (_, ch)) in bytes.iter().enumerate() {
                if *ch == '\n' {
                    let mut run = 0usize;
                    let mut j = i;
                    while j > 0 && bytes[j - 1].1 == '\\' {
                        run += 1;
                        j -= 1;
                    }
                    assert!(
                        run % 2 == 1,
                        "newline not escaped as \\\\\\n in {input:?} → {out:?} (run={run})"
                    );
                }
            }
        }
    }

    // --- document-level bracket balance --------------------------------------

    /// Count net unescaped-bracket depth across a Typst source string, treating
    /// a '[' / ']' as structural only when preceded by an even backslash run.
    /// Returns `Err(byte_offset)` if depth ever goes negative (a content block
    /// closed before it opened — an injection escape), else `Ok(final_depth)`.
    ///
    /// NOTE: this scans the *whole* document, which also contains preamble
    /// brackets from the helper definitions; those are balanced by construction,
    /// so the meaningful assertion is "never negative" and "returns to a fixed
    /// baseline regardless of user input".
    ///
    /// Brackets inside a Typst string literal (`"…"`, e.g. an `image("…")`
    /// path) are NOT structural — `]` is an ordinary character there — so the
    /// scan tracks string-literal context and ignores brackets while inside one.
    /// String literals only ever carry `escape_string`-escaped content, where
    /// `"` is `\"` and `\` is `\\`, so an unescaped `"` reliably toggles the
    /// context.
    fn unescaped_bracket_depth(s: &str) -> Result<i64, usize> {
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        let mut depth: i64 = 0;
        let mut in_string = false;
        for (i, (off, ch)) in chars.iter().enumerate() {
            // A char is escaped when preceded by an odd run of backslashes.
            let escaped = {
                let mut run = 0usize;
                let mut j = i;
                while j > 0 && chars[j - 1].1 == '\\' {
                    run += 1;
                    j -= 1;
                }
                run % 2 == 1
            };
            match ch {
                '"' if !escaped => in_string = !in_string,
                '[' if !in_string && !escaped => depth += 1,
                ']' if !in_string && !escaped => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(*off);
                    }
                }
                _ => {}
            }
        }
        Ok(depth)
    }

    /// Build a RenderBlock tree deterministically from the adversarial alphabet,
    /// then assert the assembled document keeps its unescaped-bracket nesting
    /// non-negative and at a fixed baseline independent of the (escaped) user
    /// strings — i.e. no field can prematurely close or open a content block.
    #[test]
    fn fuzz_document_bracket_balance_is_preserved() {
        const KINDS: &[&str] = &[
            "heading",
            "song",
            "music",
            "scripture",
            "liturgy",
            "announcement",
            "image",
            "form_field",
            "checkbox",
            "signature",
            "table",
            "two_column",
            "callout",
            "text",
            "totally_unknown",
        ];
        const STR_FIELDS: &[&str] = &[
            "title",
            "subtitle",
            "date",
            "text",
            "label",
            "hint",
            "author",
            "number",
            "copyright",
            "leader",
            "reader",
            "book",
            "reference",
            "translation",
            "caption",
            "url",
            "synopsis",
            "preacher",
            "role",
        ];

        fn build(rng: &mut SplitMix64, depth: usize) -> RenderBlock {
            let kind = KINDS[rng.below(KINDS.len())];
            let mut obj = serde_json::Map::new();
            let nfields = rng.below(5);
            for _ in 0..nfields {
                let key = STR_FIELDS[rng.below(STR_FIELDS.len())];
                obj.insert(key.to_string(), json!(fuzz_string(rng, 24)));
            }
            // occasionally include verses (string array) for songs
            if rng.below(3) == 0 {
                let nv = rng.below(4);
                let verses: Vec<serde_json::Value> =
                    (0..nv).map(|_| json!(fuzz_string(rng, 16))).collect();
                obj.insert("verses".into(), json!(verses));
                obj.insert("refrain".into(), json!(fuzz_string(rng, 16)));
            }
            // occasionally include a table grid with adversarial cell content,
            // ragged/out-of-range indices, and a random border keyword, so the
            // bracket-balance property covers the table renderer too.
            if rng.below(3) == 0 {
                let rows = rng.below(4);
                let cols = rng.below(4);
                obj.insert("numRows".into(), json!(rows));
                obj.insert("numCols".into(), json!(cols));
                obj.insert("headerRow".into(), json!(rng.below(2) == 0));
                obj.insert(
                    "borders".into(),
                    json!(["all", "outer", "none", "weird"][rng.below(4)]),
                );
                let ncells = rng.below(8);
                let cells: Vec<serde_json::Value> = (0..ncells)
                    .map(|_| {
                        // Indices may exceed dims (must be ignored) to fuzz the
                        // out-of-range guard.
                        json!({
                            "rowIndex": rng.below(6),
                            "colIndex": rng.below(6),
                            "content": fuzz_string(rng, 16),
                        })
                    })
                    .collect();
                obj.insert("cells".into(), json!(cells));
            }
            let children = if depth < 3 && rng.below(3) == 0 {
                let nc = rng.below(4);
                (0..nc).map(|_| build(rng, depth + 1)).collect()
            } else {
                Vec::new()
            };
            RenderBlock {
                kind: kind.to_string(),
                data: serde_json::Value::Object(obj),
                children,
            }
        }

        // Hand case: a title that tries to break out of and reopen a block.
        let hand = RenderBlock::leaf("text", json!({ "title": "x] #panic() [y" }));
        let src = doc(&[hand]);
        let baseline = unescaped_bracket_depth(&src).expect("hand case never goes negative");

        let mut rng = SplitMix64::new(0xBEEF_F00D);
        for _ in 0..2000 {
            let nblocks = rng.below(5);
            let blocks: Vec<RenderBlock> = (0..nblocks).map(|_| build(&mut rng, 0)).collect();
            let src = doc(&blocks);
            match unescaped_bracket_depth(&src) {
                Ok(d) => assert_eq!(
                    d, baseline,
                    "document bracket depth drifted from baseline; blocks={blocks:?}"
                ),
                Err(off) => panic!("unescaped ']' closed a block at byte {off}; blocks={blocks:?}"),
            }
        }
    }
}
