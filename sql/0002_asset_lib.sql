-- Phase 1.3 — extended asset library.
--
-- Adds a typed `asset_kind` column (the asset table already has a free-text
-- `kind`; this new column carries the canonical enum used by the frontend
-- grid and the `asset_lib` service) and a `tags` text column (comma-separated
-- or JSON array — stored as TEXT, parsed by the service layer).
--
-- We keep the existing `kind` column for backward compatibility with the
-- existing AssetRepo; new code goes through AssetLibRepo which reads/writes
-- these two new columns.

ALTER TABLE asset ADD COLUMN asset_kind TEXT NOT NULL DEFAULT 'Logo';
ALTER TABLE asset ADD COLUMN tags       TEXT NOT NULL DEFAULT '';
