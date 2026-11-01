# SundayPaper — product vision & layout

Moved verbatim from CLAUDE.md (2026-11-01) to keep the always-loaded agent context small.

## Core promises

1. From a service plan to a finished, printable program in one click.
2. AI compiles intent into a document — it does not just "decorate".
3. A volunteer can make Sunday's program after 10 minutes of training.
4. Member data on forms NEVER leaves the machine. Cloud AI is opt-in.
5. Mac and Windows are first-class equals.

## Competitive positioning

- vs **Adobe Acrobat**: we are task-oriented for ministry, not a generic
  page editor; 1/10 the price; AI-native; no subscription lock-in.
- vs **Canva / InDesign**: we bind to live church data (setlists, song
  catalog, sermon transcripts) — they start from a blank canvas.
- vs **manual Word/Publisher bulletins**: we generate from the plan and
  reuse a living asset library; no copy-paste every week.
- vs the old **Sangbok-Klipper**: that was one Python script; this is the
  whole publishing house, and splitting is just one feature.

## Folder structure

```
src/                   React frontend
├── app/               Route/page-level components
├── features/          Feature modules (library, splitter, builder,
│                      editor, forms, export, ...)
├── components/        Shared UI primitives
├── lib/               Utilities, hooks, IPC client, generated bindings
└── styles/            Globals, design tokens

src-tauri/             Rust backend
└── src/
    ├── commands/      Tauri command handlers (thin; delegate to services)
    ├── services/      Business logic (db, pdf, ocr, ai, layout, ...)
    ├── error.rs       Central AppError (serializes to {code, message})
    ├── lib.rs         Tauri runtime entry point
    └── main.rs

sql/                   Database migrations (versioned) — Phase 1+
docs/                  ARCHITECTURE.md, DECISIONS.md
```

