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

import type { AppError, AppInfo } from "./bindings";

const DEV = import.meta.env.DEV;
const LOG_IPC = DEV && import.meta.env.VITE_IPC_LOG !== "false";

/** Wrapper around Tauri's error that preserves the Rust `code` field. */
export class IPCError extends Error {
  readonly code: AppError["code"];
  constructor(err: AppError) {
    super(err.message);
    this.code = err.code;
    this.name = "IPCError";
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (LOG_IPC) console.debug(`[ipc] → ${cmd}`, args);
  try {
    const out = await invoke<T>(cmd, args);
    if (LOG_IPC) console.debug(`[ipc] ← ${cmd}`, out);
    return out;
  } catch (raw) {
    // Tauri rethrows a serialised AppError as a plain object
    if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
      throw new IPCError(raw as AppError);
    }
    if (raw instanceof Error) throw raw;
    throw new Error(String(raw));
  }
}

// ── App ────────────────────────────────────────────────────────────────────

export const app = {
  /** "Hello SundayPaper" — proves the Rust ↔ React bridge works. */
  info: () => call<AppInfo>("app_info"),
};

/** Bundled namespace for ergonomic imports. */
export const ipc = { app };
