import { describe, it, expect } from "vitest";

import type { DocTemplate, TemplateVar } from "@/lib/bindings";
import {
  docTemplateToForm,
  emptyDocTemplateForm,
  formToVariables,
  isDocTemplateFormValid,
  placeholdersInSource,
  starterSource,
} from "./docTemplateForm";

const mkVar = (over: Partial<TemplateVar>): TemplateVar => ({
  id: "v-1",
  template_id: "t-1",
  name: "title",
  label: "Tittel",
  kind: "Text",
  default_value: null,
  required: true,
  position: 0n,
  created_at: 0n,
  ...over,
});

const mkTemplate = (over: Partial<DocTemplate>): DocTemplate => ({
  id: "t-1",
  name: "Høymesse",
  kind: "Bulletin",
  typst_source: "{{title}}",
  preview_png: null,
  variables: [],
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

describe("docTemplateForm", () => {
  it("seeds the form from a template, sorting variables by position", () => {
    const form = docTemplateToForm(
      mkTemplate({
        variables: [
          mkVar({ name: "b", position: 1n, default_value: null }),
          mkVar({ name: "a", position: 0n, default_value: "x" }),
        ],
      }),
    );
    expect(form.name).toBe("Høymesse");
    expect(form.variables.map((v) => v.name)).toEqual(["a", "b"]);
    // null default becomes "", a non-null default is carried through
    expect(form.variables[0].defaultValue).toBe("x");
    expect(form.variables[1].defaultValue).toBe("");
  });

  it("requires a non-blank name to be valid", () => {
    expect(isDocTemplateFormValid(emptyDocTemplateForm)).toBe(false);
    expect(
      isDocTemplateFormValid({ ...emptyDocTemplateForm, name: "   " }),
    ).toBe(false);
    expect(
      isDocTemplateFormValid({ ...emptyDocTemplateForm, name: "Mal" }),
    ).toBe(true);
  });

  it("converts variable rows: drops blank-named, trims, label falls back to name", () => {
    const vars = formToVariables({
      ...emptyDocTemplateForm,
      variables: [
        {
          name: "  title  ",
          label: "  Tittel  ",
          kind: "Text",
          defaultValue: "  Høymesse  ",
          required: true,
        },
        // blank name -> dropped entirely
        {
          name: "   ",
          label: "Ignorert",
          kind: "Text",
          defaultValue: "",
          required: false,
        },
        // blank label -> falls back to the trimmed name; blank default -> null
        {
          name: "date",
          label: "",
          kind: "Date",
          defaultValue: "  ",
          required: false,
        },
      ],
    });
    expect(vars).toEqual([
      {
        name: "title",
        label: "Tittel",
        kind: "Text",
        default_value: "Høymesse",
        required: true,
      },
      {
        name: "date",
        label: "date",
        kind: "Date",
        default_value: null,
        required: false,
      },
    ]);
  });

  it("extracts placeholders in first-appearance order, de-duplicated", () => {
    expect(
      placeholdersInSource("{{title}} and {{ body }} then {{title}} again"),
    ).toEqual(["title", "body"]);
    expect(placeholdersInSource("no placeholders here")).toEqual([]);
  });

  it("starterSource embeds the title placeholder and varies by kind", () => {
    expect(starterSource("Bulletin")).toContain("{{title}}");
    expect(starterSource("Bulletin")).toContain("{{body}}");
    expect(starterSource("SongSheet")).toContain("{{songs}}");
    expect(starterSource("LargeText")).toContain("24pt");
  });
});
