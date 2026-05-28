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

## ADR-002 — Optional cargo features for heavy/native deps

- **Date:** 2026-05-28
- **Status:** accepted
- **Context:** pdfium, Tesseract and the Anthropic HTTP client are heavy and/or
  need native toolchains, keys or network. CI and contributors must be able to
  build without all of them.
- **Decision:** Gate each behind an optional cargo feature (`pdf`, `ocr`, `ai`).
  The default build compiles without them; pure logic (parsers, request builders,
  cost math) stays outside the gate and is unit-tested; the gated path stubs out
  with a clear error when the feature is off. Mirrors Verbatim/SundayStage.
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
