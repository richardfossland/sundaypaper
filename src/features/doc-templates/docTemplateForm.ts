/**
 * Pure helpers for the document-template editor form. Kept free of React so the
 * normalisation + validation rules can be unit-tested without rendering.
 *
 * Mirrors the `songForm.ts` seam: the panel binds plain strings, and these
 * helpers turn an existing `DocTemplate` into a form and a form back into the
 * shapes `ipc.docTemplate.create` / `.update` expect.
 *
 * Note the backend asymmetry: `doc_template_create` takes the full variable
 * list, but `doc_template_update` only updates name/kind/typstSource. The form
 * carries variables either way; the panel only sends them on create.
 */

import type { DocTemplate, TemplateVarInput } from "@/lib/bindings";

/** The document-template kinds the backend accepts (`DocTemplateKind`). */
export const TEMPLATE_KINDS = [
  "Bulletin",
  "SongSheet",
  "Magazine",
  "Poster",
  "Form",
  "LargeText",
] as const;
export type TemplateKind = (typeof TEMPLATE_KINDS)[number];

/** Norwegian labels for each kind, shown in the picker. */
export const KIND_LABELS: Record<TemplateKind, string> = {
  Bulletin: "Program",
  SongSheet: "Sangark",
  Magazine: "Menighetsblad",
  Poster: "Plakat",
  Form: "Skjema",
  LargeText: "Storskrift",
};

/** The variable kinds the backend accepts (`TemplateVarKind`). */
export const VAR_KINDS = [
  "Text",
  "Number",
  "Date",
  "Boolean",
  "SongList",
  "ScriptureRef",
] as const;
export type VarKind = (typeof VAR_KINDS)[number];

/** Norwegian labels for each variable kind. */
export const VAR_KIND_LABELS: Record<VarKind, string> = {
  Text: "Tekst",
  Number: "Tall",
  Date: "Dato",
  Boolean: "Ja/nei",
  SongList: "Sangliste",
  ScriptureRef: "Bibelreferanse",
};

/** A single editable variable row in the form. */
export interface VarFormRow {
  name: string;
  label: string;
  kind: string;
  defaultValue: string;
  required: boolean;
}

/** The editable fields of a document template, as the form binds them. */
export interface DocTemplateFormState {
  name: string;
  kind: string;
  typstSource: string;
  variables: VarFormRow[];
}

/** A blank variable row. */
export function emptyVarRow(): VarFormRow {
  return {
    name: "",
    label: "",
    kind: "Text",
    defaultValue: "",
    required: false,
  };
}

/** A blank form, used when composing a new template. */
export const emptyDocTemplateForm: DocTemplateFormState = {
  name: "",
  kind: "Bulletin",
  typstSource: starterSource("Bulletin"),
  variables: [],
};

/** A starter Typst skeleton so a brand-new template is not a blank page. */
export function starterSource(kind: string): string {
  const head = [
    '#set page(paper: "a4", margin: 2cm)',
    "#set text(size: 11pt)",
    "",
    '#align(center)[#text(size: 20pt, weight: "bold")[{{title}}]]',
    "",
  ];
  const body =
    kind === "SongSheet"
      ? "{{songs}}"
      : kind === "LargeText"
        ? "#set text(size: 24pt)\n{{body}}"
        : kind === "Poster"
          ? "#v(2cm)\n#align(center)[#text(size: 16pt)[{{subtitle}}]]"
          : "{{body}}";
  return [...head, body, ""].join("\n");
}

/** Seed the form from an existing template (nulls become empty strings). */
export function docTemplateToForm(t: DocTemplate): DocTemplateFormState {
  return {
    name: t.name,
    kind: t.kind,
    typstSource: t.typst_source,
    variables: [...t.variables]
      .sort((a, b) => Number(a.position) - Number(b.position))
      .map((v) => ({
        name: v.name,
        label: v.label,
        kind: v.kind,
        defaultValue: v.default_value ?? "",
        required: v.required,
      })),
  };
}

/** A template is saveable only with a non-blank name. */
export function isDocTemplateFormValid(form: DocTemplateFormState): boolean {
  return form.name.trim().length > 0;
}

/**
 * Convert the form's variable rows into the IPC `TemplateVarInput[]` shape.
 * Blank-named rows are dropped (a half-typed row should not become a variable),
 * names/labels are trimmed, and a blank label falls back to the name so the
 * fill-in UI always has something to show. Optional `defaultValue` blanks
 * become `null`.
 */
export function formToVariables(
  form: DocTemplateFormState,
): TemplateVarInput[] {
  return form.variables
    .map((v) => ({ ...v, name: v.name.trim() }))
    .filter((v) => v.name.length > 0)
    .map((v) => {
      const label = v.label.trim();
      const def = v.defaultValue.trim();
      return {
        name: v.name,
        label: label.length > 0 ? label : v.name,
        kind: v.kind,
        default_value: def.length > 0 ? def : null,
        required: v.required,
      };
    });
}

/**
 * Extract `{{name}}` placeholder names referenced in the Typst source, in first
 * appearance order, de-duplicated. Used to flag variables declared but never
 * used, and placeholders used but never declared.
 */
export function placeholdersInSource(source: string): string[] {
  const re = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g;
  const seen = new Set<string>();
  const out: string[] = [];
  for (const m of source.matchAll(re)) {
    const name = m[1];
    if (!seen.has(name)) {
      seen.add(name);
      out.push(name);
    }
  }
  return out;
}
