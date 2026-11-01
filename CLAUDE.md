# CLAUDE.md — SundayPaper

SundayPaper is the document & print companion of the Sunday suite.
It is what a church reaches for to produce ANY paper or PDF: service
programs, song sheets, the parish magazine ("menighetsblad"), large-print
sheets, posters, forms, and split/merge/OCR jobs on existing PDFs.

**Product vision, positioning and folder layout: [`docs/PRODUCT.md`](docs/PRODUCT.md)** — read it before product or UX decisions.

## The core reframe — this is NOT a generic PDF editor

- A document is NOT pages you fiddle with. It is a **TREE OF BLOCKS**
  (liturgy, song, scripture reading, announcement, image, QR) bound to
  **DATA SOURCES**. The PDF is RENDERED from that tree.
- Two directions:
  - **FORWARD** (generate): intent/data → block tree → PDF
  - **BACKWARD** (ingest): arbitrary PDF → split / OCR / merge / edit →
    feeds the ASSET LIBRARY
- A persistent **Asset Library** (logo, templates, song catalog, recurring
  blocks, fonts) means next Sunday's material is produced in seconds.

## Tech principles

- **Local-first.** Cloud is optional; member/form data stays on device.
- **Hybrid AI:** local (OCR, heuristics, sensitive work) + Claude API
  (intent→layout, drafting, translation). Every cloud call is opt-in and
  minimizes what is sent.
- **Layout engine is Typst** (templates → PDF), fast and Rust-native.
- **PDF read/render via pdfium-render; manipulation via lopdf.**
- **Languages:** English, Norwegian, Swedish, Danish, German, French,
  Polish (match SundayRec).
- **Nordic reality first:** when songs flow to SundaySong, carry
  `tono_work_id` from day one.

## Stack

- **Tauri 2** (Rust backend) + React 19 + TypeScript + Tailwind CSS v4
- **shadcn/ui** primitives as base (customized)
- **TanStack Query** (server state) + **Zustand** (UI state)
- **SQLite** via `sqlx` (local-first storage) — added in Phase 1
- **cmdk** for command palette
- **lucide-react** for icons
- **ts-rs** generates the TypeScript bindings from Rust types (run
  `cargo test export_bindings`)

## Conventions

- Tauri commands NEVER talk to `sqlx` directly — they go through services/repositories.
- All IDs are UUIDs (v7 for sortability) stored as TEXT in SQLite.
- Timestamps are i64 unix millis (avoids timezone bugs).
- Every domain entity has `created_at`, `updated_at` (soft delete where appropriate).
- Error handling: `thiserror`-based `AppError`, never `unwrap()` in production code.
- Heavy/native deps (pdfium, Tesseract, the Anthropic client) sit behind
  **optional cargo features**; the default build compiles without them and a
  stub returns a clear error. Mirrors the SundayEdit/SundayStage pattern.
- TypeScript: strict mode, no `any`, no unused vars.

## Privacy is non-negotiable

Form/member data and signatures are stored ONLY locally. No cloud AI ever
touches form content. This is a promise on par with SundayRec's "no cloud".
Make it visible in the UI.
