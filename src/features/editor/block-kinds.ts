/**
 * Block kind catalogue + JSON validation for the block editor.
 *
 * Lives in a sibling module so BlockCard.tsx only exports its component (keeps
 * React Fast Refresh working). These values are pure and self-contained.
 */

/** Block kinds the layout engine renders today (see services/bulletin.rs). */
export const BLOCK_KINDS = [
  "heading",
  "text",
  "song",
  "scripture",
  "liturgy",
  "announcement",
  // Fillable form-field kinds (Phase 7.2 — FormBuilder).
  "form_field",
  "checkbox",
  "signature",
] as const;

/** Validate a string is parseable JSON; returns an error message or null. */
export function jsonError(raw: string): string | null {
  if (raw.trim() === "") return null; // empty is allowed (defaults to "{}")
  try {
    JSON.parse(raw);
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : "Ugyldig JSON";
  }
}
