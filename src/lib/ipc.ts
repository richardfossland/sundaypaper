/**
 * Typed wrappers around Tauri's `invoke()`.
 *
 * One function per Rust command. Wraps `invoke<T>(name, args)` so:
 *   - The TypeScript caller has a stable signature
 *   - Rust `AppError` is rethrown as a JS `IPCError` the React code can catch
 *   - Dev-mode logs every call for debugging (toggle via `VITE_IPC_LOG`)
 *
 * Convention: command names are `entity_verb` (e.g. `app_info`). Matches
 * `commands::*` in Rust.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  AppError,
  AppInfo,
  Asset,
  AssetKind,
  AssetLibEntry,
  Block,
  DocTemplate,
  BatchExportResult,
  Document,
  ExportOptions,
  ImportJob,
  PdfInfo,
  LayoutMeta,
  Project,
  SangbokJob,
  ServicePlan,
  Setting,
  Song,
  Template,
  TemplateVar,
  TemplateVarInput,
} from "./bindings";

const DEV = import.meta.env.DEV;
const LOG_IPC = DEV && import.meta.env.VITE_IPC_LOG !== "false";

/** Wrapper around Tauri's error that preserves the Rust `code` field. */
export class IPCError extends Error {
  readonly code: AppError["code"];
  constructor(err: AppError, options?: ErrorOptions) {
    super(err.message, options);
    this.code = err.code;
    this.name = "IPCError";
  }
}

/** Pull a readable message out of a query/mutation error. An `IPCError` carries
 *  the Rust `code`, a plain `Error` just its message; anything else falls back. */
export function errMessage(err: unknown, fallback: string): string {
  if (err instanceof IPCError) return `${err.code} — ${err.message}`;
  if (err instanceof Error) return err.message;
  return fallback;
}

/** Map a raw value thrown by `invoke()` into a typed error. Pure + testable:
 *  Tauri rethrows a serialised `AppError` as a plain `{ code, message }`. */
export function toIPCError(raw: unknown): Error {
  if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
    return new IPCError(raw as AppError, { cause: raw });
  }
  if (raw instanceof Error) return raw;
  return new Error(String(raw), { cause: raw });
}

async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (LOG_IPC) console.debug(`[ipc] → ${cmd}`, args);
  try {
    const out = await invoke<T>(cmd, args);
    if (LOG_IPC) console.debug(`[ipc] ← ${cmd}`, out);
    return out;
  } catch (raw) {
    throw toIPCError(raw);
  }
}

// ── App ────────────────────────────────────────────────────────────────────

export const app = {
  /** "Hello SundayPaper" — proves the Rust ↔ React bridge works. */
  info: () => call<AppInfo>("app_info"),
};

// ── Projects ─────────────────────────────────────────────────────────────────

export const project = {
  create: (name: string, description?: string) =>
    call<Project>("project_create", { name, description }),
  get: (id: string) => call<Project>("project_get", { id }),
  list: () => call<Project[]>("project_list"),
  update: (id: string, name: string, description?: string) =>
    call<Project>("project_update", { id, name, description }),
  delete: (id: string) => call<void>("project_delete", { id }),
};

// ── Documents ────────────────────────────────────────────────────────────────

export const document = {
  create: (projectId: string, title: string, kind: string, pageSize?: string) =>
    call<Document>("document_create", {
      projectId,
      title,
      kind,
      pageSize,
    }),
  get: (id: string) => call<Document>("document_get", { id }),
  list: (projectId: string) => call<Document[]>("document_list", { projectId }),
  update: (id: string, title: string, kind: string, pageSize: string) =>
    call<Document>("document_update", { id, title, kind, pageSize }),
  delete: (id: string) => call<void>("document_delete", { id }),
};

// ── Blocks ───────────────────────────────────────────────────────────────────

export const block = {
  create: (
    documentId: string,
    parentId: string | null,
    kind: string,
    data?: string,
  ) => call<Block>("block_create", { documentId, parentId, kind, data }),
  get: (id: string) => call<Block>("block_get", { id }),
  list: (documentId: string) => call<Block[]>("block_list", { documentId }),
  update: (id: string, kind: string, data: string) =>
    call<Block>("block_update", { id, kind, data }),
  /** Move a block to `newPosition` within its sibling group; positions of the
   *  affected siblings renormalise to a dense `0..N` range on the backend. */
  reorder: (id: string, newPosition: number) =>
    call<Block>("block_reorder", { id, newPosition }),
  /** Move a block under a new parent (a container block), or to the top level
   *  when `newParentId` is `null`. The block lands last in the destination
   *  group; the backend rejects self-parenting and cycles. */
  reparent: (id: string, newParentId: string | null) =>
    call<Block>("block_reparent", { id, newParentId }),
  delete: (id: string) => call<void>("block_delete", { id }),
};

// ── Assets ───────────────────────────────────────────────────────────────────

export const asset = {
  create: (input: {
    kind: string;
    name: string;
    path: string;
    mime?: string;
    byteSize?: number;
    fingerprint?: string;
  }) => call<Asset>("asset_create", input),
  get: (id: string) => call<Asset>("asset_get", { id }),
  list: () => call<Asset[]>("asset_list"),
  findByFingerprint: (fingerprint: string) =>
    call<Asset | null>("asset_find_by_fingerprint", { fingerprint }),
  relink: (id: string, path: string) =>
    call<Asset>("asset_relink", { id, path }),
  delete: (id: string) => call<void>("asset_delete", { id }),
};

// ── Songs ────────────────────────────────────────────────────────────────────

export const song = {
  create: (input: {
    title: string;
    author?: string;
    body?: string;
    language?: string;
    tonoWorkId?: string;
  }) => call<Song>("song_create", input),
  get: (id: string) => call<Song>("song_get", { id }),
  list: () => call<Song[]>("song_list"),
  update: (
    id: string,
    input: {
      title: string;
      author?: string;
      body?: string;
      language?: string;
      tonoWorkId?: string;
    },
  ) => call<Song>("song_update", { id, ...input }),
  delete: (id: string) => call<void>("song_delete", { id }),
};

// ── Templates ────────────────────────────────────────────────────────────────

export const template = {
  create: (name: string, kind: string, source?: string) =>
    call<Template>("template_create", { name, kind, source }),
  get: (id: string) => call<Template>("template_get", { id }),
  list: () => call<Template[]>("template_list"),
  update: (id: string, name: string, kind: string, source: string) =>
    call<Template>("template_update", { id, name, kind, source }),
  delete: (id: string) => call<void>("template_delete", { id }),
};

// ── Import jobs ──────────────────────────────────────────────────────────────

export const importJob = {
  create: (sourcePath: string, kind: string, projectId?: string) =>
    call<ImportJob>("import_job_create", { projectId, sourcePath, kind }),
  get: (id: string) => call<ImportJob>("import_job_get", { id }),
  list: () => call<ImportJob[]>("import_job_list"),
  updateStatus: (id: string, status: string, detail?: string) =>
    call<ImportJob>("import_job_update_status", { id, status, detail }),
  /** Permanently delete a single job from the log. */
  delete: (id: string) => call<void>("import_job_delete", { id }),
  /** Delete every finished (done/errored) job. Resolves to the count removed. */
  clearFinished: () => call<number>("import_job_clear_finished"),
};

// ── Settings ─────────────────────────────────────────────────────────────────

export const setting = {
  get: (key: string) => call<string | null>("setting_get", { key }),
  set: (key: string, value: string) =>
    call<Setting>("setting_set", { key, value }),
  list: () => call<Setting[]>("setting_list"),
  delete: (key: string) => call<void>("setting_delete", { key }),
};

// ── PDF (Phase 1.2) ──────────────────────────────────────────────────────────
// Needs a build with the `pdf` cargo feature; otherwise these reject with an
// IPCError whose code is "feature_disabled".

export const pdf = {
  info: (path: string) => call<PdfInfo>("pdf_info", { path }),
  extractText: (path: string) => call<string>("pdf_extract_text", { path }),
  /** Returns a base64 PNG (no data-URL prefix). */
  renderPage: (path: string, pageIndex: number, targetWidth: number) =>
    call<string>("pdf_render_page", { path, pageIndex, targetWidth }),
  extractPages: (path: string, pages: string, outPath: string) =>
    call<void>("pdf_extract_pages", { path, pages, outPath }),
  split: (path: string, chunkSize: number, outDir: string, stem: string) =>
    call<string[]>("pdf_split", { path, chunkSize, outDir, stem }),
  merge: (inputs: string[], outPath: string) =>
    call<void>("pdf_merge", { inputs, outPath }),
  rotate: (path: string, pages: string, degrees: number, outPath: string) =>
    call<void>("pdf_rotate", { path, pages, degrees, outPath }),
};

// ── Asset Library (Phase 1.3) ────────────────────────────────────────────────
// Typed asset library with AssetKind enum + tags support. Complements the base
// `asset` namespace; the frontend can use whichever fits the context.

export const assetLib = {
  /** Register a file in the typed asset library. */
  add: (input: {
    name: string;
    kind: AssetKind;
    filePath: string;
    tags?: string;
  }) =>
    call<AssetLibEntry>("asset_add", {
      name: input.name,
      kind: input.kind,
      filePath: input.filePath,
      tags: input.tags,
    }),

  /** List live assets, optionally filtered by kind. */
  list: (kind?: AssetKind) => call<AssetLibEntry[]>("asset_list_lib", { kind }),

  /** Soft-delete an asset from the library. */
  delete: (id: string) => call<void>("asset_delete_lib", { id }),

  /** Open the asset's backing file in the system default application. */
  open: (id: string) => call<void>("asset_open", { id }),
};

// ── PDF ops (Phase 1.3) ──────────────────────────────────────────────────────
// Focused helpers for the backward-direction ingest UI.

export const pdfOps = {
  /** Return the number of pages in a PDF file. */
  pageCount: (path: string) => call<number>("pdf_page_count", { path }),
};

// ── Document templates (Phase doc-templates) ─────────────────────────────────

export const docTemplate = {
  create: (
    name: string,
    kind: string,
    typstSource?: string,
    variables?: TemplateVarInput[],
  ) =>
    call<DocTemplate>("doc_template_create", {
      name,
      kind,
      typstSource,
      variables,
    }),
  get: (id: string) => call<DocTemplate>("doc_template_get", { id }),
  list: (kind?: string) => call<DocTemplate[]>("doc_template_list", { kind }),
  update: (
    id: string,
    name: string,
    kind: string,
    typstSource: string,
    variables?: TemplateVarInput[],
  ) =>
    call<DocTemplate>("doc_template_update", {
      id,
      name,
      kind,
      typstSource,
      variables,
    }),
  delete: (id: string) => call<void>("doc_template_delete", { id }),
  /** Render a template by substituting {{VAR}} placeholders. Returns Typst source. */
  render: (id: string, vars: Record<string, string>) =>
    call<string>("doc_template_render", { id, vars }),
  /** Seed the three built-in templates. Idempotent. */
  seedBuiltins: () => call<void>("doc_template_seed_builtins"),
};

// ── Bulletin (SundayPlan → program bridge) ───────────────────────────────────
// The FORWARD pipeline: a service plan becomes a `program` document of blocks
// (`generate`), and that block tree renders to Typst source (`render`).

export const bulletin = {
  /** Generate a `program` document from a planned service. Returns the new doc;
   *  list its blocks via `ipc.block.list`. */
  generate: (projectId: string, plan: ServicePlan, title?: string) =>
    call<Document>("bulletin_generate", { projectId, plan, title }),
  /** Generate a `program` document from a *canonical* SundayPlan service plan
   *  handed over as a JSON string (already fetched from SundayPlan, or pasted by
   *  the operator — no network fetch happens here). The backend deserialises the
   *  published `sunday-contracts` ServicePlan, runs the pure Plan→Paper adapter,
   *  and persists the blocks. Returns the new doc; list its blocks via
   *  `ipc.block.list`. Rejects malformed JSON with a "json" IPCError and an empty
   *  plan with a "validation" IPCError. */
  generateFromPlan: (projectId: string, planJson: string, title?: string) =>
    call<Document>("bulletin_generate_from_plan", {
      projectId,
      planJson,
      title,
    }),
  /** Render a document's block tree to Typst source. `layoutMeta` is optional;
   *  when omitted the document's page size seeds the page metadata. */
  render: (docId: string, layoutMeta?: LayoutMeta) =>
    call<string>("bulletin_render", { documentId: docId, layoutMeta }),
  /** Compile Typst source to a PDF, returned as base64 (no data-URL prefix) so
   *  it can drop straight into a download or `<embed src="data:…;base64,…">`.
   *  Needs a build with the `typst` cargo feature; otherwise rejects with an
   *  IPCError whose code is "feature_disabled". */
  typstCompile: (source: string) => call<string>("typst_compile", { source }),
};

// ── AI intent→layout compiler (Phase 5.1) ───────────────────────────────────
// Free-text intent → a populated `program` document of blocks, via Claude's
// tool-use (constrained to the existing block catalogue) flowing into the same
// build→layout→Typst pipeline. Consent-gated (the "Sky-AI (Claude)" privacy
// toggle) + needs an Anthropic key in Settings. Without consent/key it rejects
// with a "validation" IPCError; a build without the `ai` cargo feature rejects
// with "feature_disabled" ("AI ikke aktivert"). The manual builder is
// unaffected either way.

export const ai = {
  /** Compile a free-text intent into a `program` document. `church` / `date` /
   *  `lang` are optional context shared with the model (never form/member
   *  data). Returns the new doc; list its blocks via `ipc.block.list`. */
  compileIntent: (
    projectId: string,
    intent: string,
    opts?: {
      title?: string;
      church?: string;
      date?: string;
      lang?: string;
    },
  ) =>
    call<Document>("ai_compile_intent", {
      projectId,
      intent,
      title: opts?.title,
      church: opts?.church,
      date: opts?.date,
      lang: opts?.lang,
    }),
};

// ── Batch export (Phase 6) ───────────────────────────────────────────────────
// Render a set of documents to PDF files in one pass, applying one set of
// options (paper size, large-print scaling, language) to all of them. Reuses
// the exact render→compile chain the Builder/Editor run. Needs a build with the
// `typst` cargo feature; otherwise rejects with a "feature_disabled" IPCError.

export const exporter = {
  /** Render `documentIds` to PDFs in `outDir`. Returns where the files landed
   *  plus one entry per written PDF, in request order. */
  batch: (documentIds: string[], options: ExportOptions, outDir: string) =>
    call<BatchExportResult>("bulletin_batch_export", {
      documentIds,
      options,
      outDir,
    }),
};

// ── Sangbok-klipper (Phase 3.1 OCR prep) ─────────────────────────────────────

export const sangbok = {
  /** Queue and process (stub) an OCR job for the given PDF path. */
  import: (pdfPath: string) => call<SangbokJob>("sangbok_import", { pdfPath }),
  listJobs: () => call<SangbokJob[]>("sangbok_list_jobs"),
  getJob: (id: string) => call<SangbokJob>("sangbok_get_job", { id }),
  cancel: (id: string) => call<SangbokJob>("sangbok_cancel", { id }),
};

/** Bundled namespace for ergonomic imports. */
export const ipc = {
  app,
  project,
  document,
  block,
  asset,
  assetLib,
  song,
  template,
  docTemplate,
  importJob,
  setting,
  pdf,
  pdfOps,
  bulletin,
  ai,
  exporter,
  sangbok,
};

// Re-export TemplateVar so panels can import from here.
export type { TemplateVar };
