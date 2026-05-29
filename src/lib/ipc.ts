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
  Block,
  Document,
  ImportJob,
  Project,
  Setting,
  Song,
  Template,
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
};

// ── Settings ─────────────────────────────────────────────────────────────────

export const setting = {
  get: (key: string) => call<string | null>("setting_get", { key }),
  set: (key: string, value: string) =>
    call<Setting>("setting_set", { key, value }),
  list: () => call<Setting[]>("setting_list"),
  delete: (key: string) => call<void>("setting_delete", { key }),
};

/** Bundled namespace for ergonomic imports. */
export const ipc = {
  app,
  project,
  document,
  block,
  asset,
  song,
  template,
  importJob,
  setting,
};
