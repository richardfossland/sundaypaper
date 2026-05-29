-- Phase 1.1 — core data model.
--
-- A document is a TREE OF BLOCKS bound to data sources; the PDF is rendered
-- from that tree (see docs/ARCHITECTURE.md). This migration lays down the
-- persistent spine: projects own documents, documents own a block tree, and a
-- shared Asset Library (templates, assets, songs) makes next Sunday fast.
--
-- Conventions (CLAUDE.md): all ids are UUIDv7 stored as TEXT; timestamps are
-- i64 unix-millis; every entity carries created_at/updated_at and soft-deletes
-- via deleted_at where appropriate. Foreign keys are enforced (PRAGMA set on
-- every connection in services::db).

-- Top-level container grouping related documents and their work.
CREATE TABLE project (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

-- Reusable layout template (Typst source + typed variable interface, Phase 4.2).
CREATE TABLE template (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,                  -- program | song_sheet | poster | ...
    source      TEXT NOT NULL DEFAULT '',       -- Typst source; empty until Phase 4
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

-- A file asset in the library: logo, image, font, scanned PDF, ... Local-first:
-- `path` is an absolute path on THIS device; `fingerprint` is the O(1) content
-- fingerprint used to relink moved files (same pattern as Verbatim/SundayStage).
CREATE TABLE asset (
    id          TEXT PRIMARY KEY NOT NULL,
    kind        TEXT NOT NULL,                  -- image | font | pdf | logo | ...
    name        TEXT NOT NULL,
    path        TEXT NOT NULL,
    mime        TEXT,
    byte_size   INTEGER,
    fingerprint TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

-- A song in the catalog. tono_work_id is first-class from day one so songs can
-- flow to SundaySong with their Nordic rights id intact (Phase 8).
CREATE TABLE song (
    id           TEXT PRIMARY KEY NOT NULL,
    title        TEXT NOT NULL,
    author       TEXT,
    body         TEXT NOT NULL DEFAULT '',
    language     TEXT,
    tono_work_id TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    deleted_at   INTEGER
);

-- A document rendered from a block tree. Belongs to a project; optionally bound
-- to a template. `position` orders documents within their project.
CREATE TABLE document (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    template_id TEXT REFERENCES template(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    kind        TEXT NOT NULL,                  -- program | song_sheet | magazine | poster | form
    page_size   TEXT NOT NULL DEFAULT 'A4',
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

-- A node in a document's block tree. parent_id NULL = top level. `data` holds a
-- kind-specific JSON payload (liturgy text, song ref, image asset id, ...).
CREATE TABLE block (
    id          TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES document(id) ON DELETE CASCADE,
    parent_id   TEXT REFERENCES block(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,                  -- liturgy | song | scripture | announcement | image | qr
    position    INTEGER NOT NULL DEFAULT 0,
    data        TEXT NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- A backward-direction ingest job: split / OCR / merge on an existing PDF
-- (Phase 1.2+). Optionally scoped to a project.
CREATE TABLE import_job (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT REFERENCES project(id) ON DELETE SET NULL,
    source_path TEXT NOT NULL,
    kind        TEXT NOT NULL,                  -- split | ocr | merge
    status      TEXT NOT NULL DEFAULT 'pending',-- pending | running | done | error
    detail      TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- Simple local-only key/value app settings (locale, last project, ...).
CREATE TABLE setting (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_document_project ON document(project_id);
CREATE INDEX idx_block_document   ON block(document_id);
CREATE INDEX idx_block_parent     ON block(parent_id);
CREATE INDEX idx_import_project   ON import_job(project_id);
CREATE INDEX idx_song_tono        ON song(tono_work_id);
