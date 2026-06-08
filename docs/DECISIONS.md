# Architecture Decision Records

Lightweight ADRs. Newest first. Copy the template for each new decision.

---

## Template

```
## ADR-NNN — <short title>
- **Date:** YYYY-MM-DD
- **Status:** proposed | accepted | superseded by ADR-MMM
- **Context:** what forces the decision (constraints, requirements, trade-offs)
- **Decision:** what we chose
- **Consequences:** what becomes easier/harder; follow-ups
```

---

## ADR-003 — Runtime-checked sqlx queries; migrations in `sql/`

- **Date:** 2026-05-29
- **Status:** accepted
- **Context:** sqlx can verify SQL at compile time, but that needs a live
  `DATABASE_URL` or a committed `.sqlx` offline cache — extra ceremony that
  breaks fresh checkouts and CI without a database. SundayPaper is local-first
  with a schema small enough to verify another way.
- **Decision:** Use runtime-checked queries (`sqlx::query` / `query_as` with
  `#[derive(FromRow)]`), not the compile-time `query!` macros. The schema is the
  versioned migrations in `sql/`, embedded via `sqlx::migrate!` and applied on
  connect; correctness is guarded by repo unit tests that run every migration
  against an in-memory SQLite db. Foreign keys are turned on per connection.
- **Consequences:** Builds and CI need no database or `DATABASE_URL`; fresh
  clones just work. The trade-off is that a typo in SQL surfaces in tests rather
  than at compile time — acceptable given each repo ships with tests. IDs are
  UUIDv7 TEXT; timestamps are i64 unix-ms (`services::db::now_ms`).

## ADR-002 — Optional cargo features for heavy/native deps

- **Date:** 2026-05-28
- **Status:** accepted
- **Context:** pdfium, Tesseract and the Anthropic HTTP client are heavy and/or
  need native toolchains, keys or network. CI and contributors must be able to
  build without all of them.
- **Decision:** Gate each behind an optional cargo feature (`pdf`, `ocr`, `ai`).
  The default build compiles without them; pure logic (parsers, request builders,
  cost math) stays outside the gate and is unit-tested; the gated path stubs out
  with a clear error when the feature is off. Mirrors SundayEdit/SundayStage.
- **Consequences:** Fast default builds and green CI without secrets. Real
  functionality needs `--features …`. Distribution (Phase 9) builds with the
  full feature set + bundled binaries.

## ADR-001 — Tauri 2 over Electron; mirror SundayStage's stack

- **Date:** 2026-05-28
- **Status:** accepted
- **Context:** SundayPaper must run well on modest volunteer machines and be a
  first-class equal on Mac and Windows. It is one of several Sunday-suite desktop
  apps; consistency lowers maintenance cost.
- **Decision:** Tauri 2 (Rust) + React 19 + TS + Tailwind v4 (CSS-first
  `@theme`), with cmdk, TanStack Query, ts-rs bindings, and SQLite via sqlx.
  Folder structure and conventions mirror SundayStage exactly.
- **Consequences:** Small binaries, low memory, shared patterns across the suite.
  Layout engine is Typst (Rust-native) rather than an HTML-to-PDF path.
