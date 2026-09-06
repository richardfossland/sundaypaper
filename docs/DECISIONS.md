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

## ADR-004 — TypeScript 7 via `@typescript/native`; the `typescript` name stays on the 6.0 API

- **Date:** 2026-09-06
- **Status:** accepted
- **Context:** TypeScript 7 is the native (Go) compiler. It ships as
  platform-specific binaries and no longer exposes the old JavaScript compiler
  API in the shape tools depend on. `typescript-eslint` (all channels, incl.
  canary, as of 8.69.0) declares `typescript: ">=4.8.4 <6.1.0"` and hard-throws
  at import time on TS >= 7:
  `Error: typescript-eslint does not support TS 7.0.`
  So "install typescript@7" and "run eslint" are mutually exclusive as long as
  a single package named `typescript` has to serve both.
- **Decision:** Use the side-by-side layout Microsoft documents in the TS 7.0
  announcement — two npm aliases instead of one dependency:
  ```json
  "@typescript/native": "npm:typescript@~7.0.2",
  "typescript":         "npm:@typescript/typescript6@^6.0.2"
  ```
  `node_modules/.bin/tsc` therefore resolves to TS 7 (used by `npm run
typecheck` and `npm run build`), while `require("typescript")` resolves to
  the TS 6.0 API that `typescript-eslint` parses with. The compat package
  deliberately names its binary `tsc6` so the two never collide.
- **Consequences:**
  - **The `"typescript": "npm:@typescript/typescript6@..."` line is not a
    downgrade.** It is the parser API for eslint. The compiler is
    `@typescript/native`. Do not "fix" it back to `"typescript": "^7"` — that
    re-breaks `npm run lint`.
  - Type _checking_ is TS 7; eslint _parsing_ is TS 6. Syntax that only TS 7
    understands would typecheck but fail to lint. We use no such syntax today.
  - We use `tseslint.configs.recommended`, not `recommendedTypeChecked`, so the
    TS 6 API is used for parsing only — no type-aware rule reads TS 6 semantics
    while `tsc` reads TS 7. Turning on type-aware linting later would make that
    split meaningful and should be revisited then.
  - Revisit when typescript-eslint ships TS >= 7.1 support
    (typescript-eslint#10940). At that point both aliases collapse back into a
    single `"typescript": "^7"`.
  - `baseUrl` is a removed option in TS 7; `paths` targets are now relative to
    the tsconfig and need a leading `./`. See `tsconfig.json`.

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
