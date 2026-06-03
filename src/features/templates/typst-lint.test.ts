/**
 * Pure-helper tests for the template builder. No React, no backend — just the
 * placeholder extraction, sample-data injection, and the cheap Typst lint that
 * guards a compile round-trip.
 */
import { describe, it, expect } from "vitest";

import {
  extractVariables,
  injectSampleData,
  validateTypst,
} from "./typst-lint";

describe("extractVariables", () => {
  it("collects placeholders de-duplicated, in first-seen order, with counts", () => {
    const src = "{{ title }} — {{ church_name }}\n{{ title }} again";
    expect(extractVariables(src)).toEqual([
      { name: "title", count: 2 },
      { name: "church_name", count: 1 },
    ]);
  });

  it("tolerates loose whitespace inside the braces", () => {
    expect(extractVariables("{{title}} {{   date   }}")).toEqual([
      { name: "title", count: 1 },
      { name: "date", count: 1 },
    ]);
  });

  it("returns nothing for source without placeholders", () => {
    expect(extractVariables('#set page(paper: "a4")')).toEqual([]);
  });
});

describe("injectSampleData", () => {
  it("substitutes provided sample values", () => {
    const out = injectSampleData("Velkommen til {{ church_name }}", {
      church_name: "Domkirken",
    });
    expect(out).toBe("Velkommen til Domkirken");
  });

  it("falls back to a visible [name] marker for unfilled slots", () => {
    expect(
      injectSampleData("{{ title }} / {{ missing }}", { title: "T" }),
    ).toBe("T / [missing]");
  });

  it("replaces every occurrence of a repeated variable", () => {
    expect(injectSampleData("{{ x }}-{{ x }}", { x: "1" })).toBe("1-1");
  });
});

describe("validateTypst", () => {
  it("passes clean source with placeholders intact", () => {
    const src = '#align(center)[#text(weight: "bold")[{{ title }}]]';
    expect(validateTypst(src)).toEqual([]);
  });

  it("flags an unbalanced curly brace", () => {
    const issues = validateTypst("#align(center)[{{ title }}");
    expect(issues.some((i) => i.severity === "error")).toBe(true);
    expect(issues.map((i) => i.message).join(" ")).toMatch(/hakeparentes/);
  });

  it("flags an extra closing paren", () => {
    const issues = validateTypst("#set text(size: 11pt))");
    expect(issues.some((i) => /parentes/.test(i.message))).toBe(true);
  });

  it("flags a malformed placeholder", () => {
    const issues = validateTypst("Hei {{ }}");
    expect(issues.some((i) => i.severity === "error")).toBe(true);
  });

  it("warns on a dangling # with no expression after it", () => {
    const issues = validateTypst("text # \nmore");
    expect(issues.some((i) => i.severity === "warning")).toBe(true);
  });

  it("does not count placeholder braces against the balance check", () => {
    // Two `{{ }}` placeholders supply 4 braces that must NOT be flagged.
    expect(validateTypst("{{ a }} and {{ b }}")).toEqual([]);
  });
});
