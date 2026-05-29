# SundayPaper — Architecture

> Skeleton established in Phase 0. Each phase fills in its section.

## The core reframe

A **document is a tree of blocks bound to data sources**; the PDF is _rendered_
from that tree. SundayPaper works in two directions:

- **FORWARD** (generate): intent / data → block tree → PDF
- **BACKWARD** (ingest): arbitrary PDF → split / OCR / merge / edit → feeds the
  **Asset Library**

The Asset Library (logos, templates, song catalog, reusable block snippets,
fonts) is the persistent core that makes next Sunday's material fast.

## Process & layers

```
┌──────────────────────────────────────────────┐
│ React 19 + TS (Vite)                          │  UI: app shell, features,
│  features/ · components/ · lib/ipc.ts         │  design tokens, ⌘K palette
└───────────────┬──────────────────────────────┘
                │ Tauri IPC (invoke) — typed via ts-rs bindings
┌───────────────▼──────────────────────────────┐
│ Rust (Tauri 2)                                │
│  commands/  thin IPC handlers                 │
│  services/  pdf · ocr · layout · ai · db      │  business logic
│  error.rs   AppError → { code, message }      │
└───────────────┬──────────────────────────────┘
                │
        SQLite (sqlx) · pdfium · Tesseract · Typst · Claude API
```

- **Commands never touch sqlx directly** — they go through services/repositories.
- **Errors** are a single `thiserror` enum that serializes to `{ code, message }`
  so the renderer can pattern-match; production code never `unwrap()`s.
- **Heavy/native deps** (pdfium, Tesseract, the Anthropic client) sit behind
  **optional cargo features**; the default build compiles without them and a stub
  returns a clear error.

## Data model

_Phase 1.1 — landed._ Schema in `sql/0001_init.sql`. Entities: `project`,
`document`, `block`, `template`, `asset`, `song` (with nullable `tono_work_id`
from day one), `import_job`, `setting`. UUIDv7 TEXT ids, i64 unix-ms timestamps,
FKs enforced on every connection. Every entity has a repository in
`services::*` — `project`, `document`, `block`, `asset`, `song`, `template`,
`import_job`, `setting` — each create / get / list / update plus the right
delete (soft-delete for library content; hard-delete with cascade for `block`
subtrees; `setting` is a plain kv upsert; `import_job` is an append-only job log
with `update_status`). All are covered by unit tests against an in-memory db.
Queries are runtime-checked (`sqlx::query` / `query_as`), so no compile-time
`DATABASE_URL` is required (see ADR-003).

## PDF layer

_Phase 1.2._ Rendering + text extraction via `pdfium-render`; low-level
manipulation (split/merge/rotate/extract) via `lopdf`. Clean trait boundary so
implementations can be swapped.

## Layout engine

_Phase 4.2._ Typst (embedded compiler crate). Templates are Typst source with a
typed variable interface; the resolved block tree is injected and compiled to PDF
in-process.

## AI layer

_Phase 5.1._ Hybrid: local heuristics/OCR for sensitive + offline work; Claude
API (opt-in, BYOK via the system keychain) for intent→layout, drafting,
translation. Every cloud call carries a `purpose` tag and is consent-gated.
**Form/member data never leaves the device.**

## Cross-product integration

_Phase 8._ Shared Sunday account (OIDC). Magic moments: SundayPlan setlist →
program; scanned songbook → SundaySong (carry `tono_work_id`); song → SundayStage
slides; SundayRec transcript → magazine recap + episode QR.

## Performance budgets

- App start: < 2 s cold
- Live preview re-render of a typical program: < 1 s
- Library open with thousands of assets: < 500 ms to interactive
