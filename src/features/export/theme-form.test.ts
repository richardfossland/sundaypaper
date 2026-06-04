/**
 * theme-form unit tests — the pure draft → `LayoutTheme | null` logic behind
 * the export panel's per-church branding (Step 3). Mirrors the backend's
 * validation posture: blank/default fields become null (keeping the unthemed,
 * byte-identical preamble), an invalid accent is surfaced + dropped, and the
 * spacing multiplier is clamped to the backend's range.
 */
import { describe, it, expect } from "vitest";

import {
  SPACING_MAX,
  SPACING_MIN,
  accentError,
  buildTheme,
  clampSpacing,
  emptyThemeDraft,
  isHeadingWeight,
  normalizeAccent,
} from "./theme-form";

describe("buildTheme", () => {
  it("returns null when the theme is disabled", () => {
    const draft = { ...emptyThemeDraft(), headingFont: "Inter" };
    expect(buildTheme(draft)).toBeNull();
  });

  it("returns null when enabled but every field is at the house default", () => {
    // Toggled on but untouched → byte-identical unthemed output, so send null.
    expect(buildTheme({ ...emptyThemeDraft(), enabled: true })).toBeNull();
  });

  it("carries only the fields the user set, blanks become null", () => {
    const theme = buildTheme({
      enabled: true,
      headingFont: "Montserrat",
      bodyFont: "  ", // blank → null
      accentColor: "#C81E2D",
      headingWeight: "black",
      spacingMultiplier: 1.5,
    });
    expect(theme).toEqual({
      headingFont: "Montserrat",
      bodyFont: null,
      accentColor: "#c81e2d", // canonicalised lowercase
      headingWeight: "black",
      spacingMultiplier: 1.5,
    });
  });

  it("treats the default weight bold and 1.0 spacing as unset (null)", () => {
    const theme = buildTheme({
      enabled: true,
      headingFont: "Inter",
      bodyFont: "",
      accentColor: "",
      headingWeight: "bold",
      spacingMultiplier: 1.0,
    });
    expect(theme).toEqual({
      headingFont: "Inter",
      bodyFont: null,
      accentColor: null,
      headingWeight: null,
      spacingMultiplier: null,
    });
  });

  it("drops an invalid accent (sends null) while keeping other fields", () => {
    const theme = buildTheme({
      ...emptyThemeDraft(),
      enabled: true,
      headingFont: "Inter",
      accentColor: 'not-a-color"); #panic()',
    });
    expect(theme).not.toBeNull();
    expect(theme!.accentColor).toBeNull();
    expect(theme!.headingFont).toBe("Inter");
  });

  it("clamps an out-of-range spacing multiplier", () => {
    const theme = buildTheme({
      ...emptyThemeDraft(),
      enabled: true,
      spacingMultiplier: 99,
    });
    expect(theme!.spacingMultiplier).toBe(SPACING_MAX);
  });
});

describe("normalizeAccent", () => {
  it("accepts #rgb and #rrggbb and lowercases", () => {
    expect(normalizeAccent("#FA0")).toBe("#fa0");
    expect(normalizeAccent("  #C81E2D ")).toBe("#c81e2d");
  });

  it("returns null for blank or invalid hex", () => {
    expect(normalizeAccent("")).toBeNull();
    expect(normalizeAccent("red")).toBeNull();
    expect(normalizeAccent("#12")).toBeNull();
    expect(normalizeAccent("#12345")).toBeNull();
    expect(normalizeAccent("#xyzxyz")).toBeNull();
  });
});

describe("accentError", () => {
  it("is null for blank (unset) input", () => {
    expect(accentError("   ")).toBeNull();
  });
  it("is null for a valid hex colour", () => {
    expect(accentError("#abc")).toBeNull();
  });
  it("is a message for an invalid colour", () => {
    expect(accentError("blue")).not.toBeNull();
  });
});

describe("isHeadingWeight", () => {
  it("accepts the known set, rejects others", () => {
    expect(isHeadingWeight("semibold")).toBe(true);
    expect(isHeadingWeight("ultralight")).toBe(false);
  });
});

describe("clampSpacing", () => {
  it("clamps to the backend range and handles non-finite", () => {
    expect(clampSpacing(0.1)).toBe(SPACING_MIN);
    expect(clampSpacing(10)).toBe(SPACING_MAX);
    expect(clampSpacing(1.25)).toBe(1.25);
    expect(clampSpacing(Number.NaN)).toBe(1.0);
  });
});
