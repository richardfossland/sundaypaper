/**
 * Theme-form logic for the batch-export options panel (Step 3: typography /
 * per-church branding).
 *
 * The UI collects raw strings (font names, an accent hex, a weight keyword, a
 * spacing slider) and an on/off toggle. This module turns that draft into the
 * `LayoutTheme | null` the backend expects:
 *
 *   - when the toggle is off → `null` (the house default; an unthemed,
 *     byte-identical preamble),
 *   - when on → a `LayoutTheme` carrying only the fields the user actually set
 *     (blank inputs become `null` so a partial theme keeps house defaults).
 *
 * The backend re-validates and falls back per-field (a bad font/colour can
 * never inject markup), so this layer is purely about not sending obvious
 * noise and giving the user inline feedback. Kept pure (no React) so it is
 * unit-testable and BlockCard-style Fast-Refresh-safe.
 */

import type { LayoutTheme } from "@/lib/bindings";

/** The weight keywords the backend accepts, in display order. */
export const HEADING_WEIGHTS = [
  "regular",
  "medium",
  "semibold",
  "bold",
  "black",
] as const;

export type HeadingWeight = (typeof HEADING_WEIGHTS)[number];

/** Spacing multiplier bounds — mirror the backend's clamp (0.5–3.0). */
export const SPACING_MIN = 0.5;
export const SPACING_MAX = 3.0;

/** The raw, editable form state the panel keeps. */
export interface ThemeDraft {
  enabled: boolean;
  headingFont: string;
  bodyFont: string;
  accentColor: string;
  headingWeight: HeadingWeight;
  spacingMultiplier: number;
}

/** A fresh, house-default draft (theme off, neutral values). */
export function emptyThemeDraft(): ThemeDraft {
  return {
    enabled: false,
    headingFont: "",
    bodyFont: "",
    accentColor: "",
    headingWeight: "bold",
    spacingMultiplier: 1.0,
  };
}

/** Is this a heading-weight keyword the backend accepts? */
export function isHeadingWeight(value: string): value is HeadingWeight {
  return (HEADING_WEIGHTS as readonly string[]).includes(value);
}

/**
 * Validate an accent-colour input the same way the backend does: a `#rgb` or
 * `#rrggbb` hex string. Returns the canonical lowercase form, or `null` if the
 * (non-blank) input is not a valid hex colour. A blank input is "unset" and
 * also returns `null` — callers distinguish the two via `accentError`.
 */
export function normalizeAccent(raw: string): string | null {
  const s = raw.trim();
  if (s === "") return null;
  const m = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.exec(s);
  return m ? `#${m[1].toLowerCase()}` : null;
}

/**
 * A user-facing error for the accent field, or `null` when it is blank or
 * valid. (Blank = "leave default", not an error.)
 */
export function accentError(raw: string): string | null {
  if (raw.trim() === "") return null;
  return normalizeAccent(raw) === null
    ? "Bruk en hex-farge som #C81E2D eller #FA0."
    : null;
}

/** Trim a font-name input to the value the backend should see, or `null`. */
function fontOrNull(raw: string): string | null {
  const s = raw.trim();
  return s === "" ? null : s;
}

/**
 * Build the `LayoutTheme | null` to send from a draft. `null` when the theme is
 * off OR when (enabled but) every field is at its house default — so a toggled-
 * on-but-untouched theme still produces the byte-identical unthemed output.
 *
 * An invalid accent is dropped (sent as `null`); the inline `accentError`
 * surfaces it to the user, and the backend would drop it anyway.
 */
export function buildTheme(draft: ThemeDraft): LayoutTheme | null {
  if (!draft.enabled) return null;

  const headingFont = fontOrNull(draft.headingFont);
  const bodyFont = fontOrNull(draft.bodyFont);
  const accentColor = normalizeAccent(draft.accentColor);
  // A weight other than the default "bold" counts as set; "bold" is the house
  // default so we send null and keep the preamble byte-identical.
  const headingWeight =
    draft.headingWeight === "bold" ? null : draft.headingWeight;
  // Likewise 1.0 is the no-op default.
  const spacingMultiplier =
    Math.abs(draft.spacingMultiplier - 1.0) < 1e-9
      ? null
      : clampSpacing(draft.spacingMultiplier);

  const allDefault =
    headingFont === null &&
    bodyFont === null &&
    accentColor === null &&
    headingWeight === null &&
    spacingMultiplier === null;
  if (allDefault) return null;

  return {
    headingFont,
    bodyFont,
    accentColor,
    headingWeight,
    spacingMultiplier,
  };
}

/** Clamp a spacing multiplier into the backend's accepted range. */
export function clampSpacing(value: number): number {
  if (!Number.isFinite(value)) return 1.0;
  return Math.min(SPACING_MAX, Math.max(SPACING_MIN, value));
}
