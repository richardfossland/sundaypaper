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

import type { AppError, AppInfo, Document, Project } from "./bindings";

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

/** Bundled namespace for ergonomic imports. */
export const ipc = { app, project, document };
