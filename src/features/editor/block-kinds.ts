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
  // Structured grid: service orders, rosters, schedules, magazine grids.
  "table",
  // Container kinds (Step 2): they nest OTHER blocks as children.
  //  - two_column: a 1fr/1fr grid (poetry-on-left / translation-on-right).
  //  - callout: a boxed/highlighted region (prayers, notes, asides).
  "two_column",
  "callout",
] as const;

/**
 * Block kinds that arrange their *children* instead of their own `data` — the
 * renderer recurses into them (see markup.rs render_two_column / render_callout).
 * The editor uses this to offer a "nest under" affordance and to label the card.
 */
export const CONTAINER_KINDS = ["two_column", "callout"] as const;

/** Is this kind a container (nests child blocks)? */
export function isContainerKind(kind: string): boolean {
  return (CONTAINER_KINDS as readonly string[]).includes(kind);
}

/** Human (Norwegian) label for a block kind, used in selectors/menus. */
export function blockKindLabel(kind: string): string {
  switch (kind) {
    case "two_column":
      return "To kolonner";
    case "callout":
      return "Faktaboks";
    default:
      return kind;
  }
}

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
