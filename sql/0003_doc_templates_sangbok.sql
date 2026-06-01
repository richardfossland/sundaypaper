-- Phase: Document template system + Sangbok-klipper pipeline.
--
-- doc_template  : typed document templates with Typst source + variable spec
-- template_var  : variable definitions per template (kind, label, default, required)
-- sangbok_job   : OCR pipeline job per imported PDF sangbok
-- song_extract  : per-page song extract found (or stub) per job

-- Typed document template (extends the basic `template` table concept but is
-- a distinct entity with richer schema).
CREATE TABLE doc_template (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,   -- Bulletin|SongSheet|Magazine|Poster|Form|LargeText
    typst_source TEXT NOT NULL DEFAULT '',
    -- preview_png stored as BLOB (NULL until generated)
    preview_png BLOB,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

-- Variable definitions belonging to a doc_template.
CREATE TABLE template_var (
    id            TEXT PRIMARY KEY NOT NULL,
    template_id   TEXT NOT NULL REFERENCES doc_template(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,           -- variable key used in Typst source
    label         TEXT NOT NULL,           -- human-facing label
    kind          TEXT NOT NULL,           -- Text|Number|Date|Boolean|SongList|ScriptureRef
    default_value TEXT,
    required      INTEGER NOT NULL DEFAULT 0,  -- 0 = false, 1 = true
    position      INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL
);

-- Sangbok OCR import job.
CREATE TABLE sangbok_job (
    id              TEXT PRIMARY KEY NOT NULL,
    input_pdf_path  TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'Queued', -- Queued|Processing|Done|Failed
    page_count      INTEGER NOT NULL DEFAULT 0,
    error_detail    TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- Song extracts discovered (or stubbed) for a sangbok_job.
CREATE TABLE song_extract (
    id           TEXT PRIMARY KEY NOT NULL,
    job_id       TEXT NOT NULL REFERENCES sangbok_job(id) ON DELETE CASCADE,
    page_start   INTEGER NOT NULL,
    page_end     INTEGER NOT NULL,
    title_hint   TEXT NOT NULL DEFAULT '',
    confidence   REAL NOT NULL DEFAULT 0.0,
    position     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_template_var_template ON template_var(template_id);
CREATE INDEX idx_sangbok_job_status    ON sangbok_job(status);
CREATE INDEX idx_song_extract_job      ON song_extract(job_id);
