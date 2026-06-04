/**
 * Pure view helpers for the import-job (OCR/ingest) history panel. Kept free of
 * React so the status mapping + sorting + filtering rules can be unit-tested.
 *
 * The backend exposes `import_job_list` / `_get` / `_create` /
 * `_update_status` — there is NO delete or clear command, so "clearing" here is
 * a client-side view filter (hide finished jobs), never a destructive DB op.
 */

import type { ImportJob } from "@/lib/bindings";

/** The status values the backend documents: pending | running | done | error. */
export type JobStatus = "pending" | "running" | "done" | "error";

/** Normalise a raw status string to a known status, defaulting to pending. */
export function normStatus(raw: string): JobStatus {
  const s = raw.toLowerCase();
  if (s === "running") return "running";
  if (s === "done") return "done";
  if (s === "error") return "error";
  return "pending";
}

/** Norwegian label for a status badge. */
export function statusLabel(raw: string): string {
  switch (normStatus(raw)) {
    case "running":
      return "Kjører";
    case "done":
      return "Ferdig";
    case "error":
      return "Feilet";
    default:
      return "Venter";
  }
}

/** A status is "finished" (terminal) when it is done or errored. */
export function isFinished(raw: string): boolean {
  const s = normStatus(raw);
  return s === "done" || s === "error";
}

/** Norwegian label for a job kind, falling back to the raw value. */
export function kindLabel(kind: string): string {
  const map: Record<string, string> = {
    ocr: "OCR",
    split: "Oppdeling",
    merge: "Sammenslåing",
    import: "Import",
  };
  return map[kind.toLowerCase()] ?? kind;
}

/** Just the file name from a source path (handles / and \ separators). */
export function baseName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

/**
 * Sort jobs newest-first by `created_at`, and optionally drop finished ones.
 * Pure: returns a new array, does not mutate the input.
 */
export function viewJobs(
  jobs: ImportJob[],
  opts: { hideFinished: boolean },
): ImportJob[] {
  const list = opts.hideFinished
    ? jobs.filter((j) => !isFinished(j.status))
    : [...jobs];
  return list.sort((a, b) => Number(b.created_at) - Number(a.created_at));
}

/** Count how many jobs are in each status bucket. */
export function statusCounts(jobs: ImportJob[]): Record<JobStatus, number> {
  const counts: Record<JobStatus, number> = {
    pending: 0,
    running: 0,
    done: 0,
    error: 0,
  };
  for (const j of jobs) counts[normStatus(j.status)] += 1;
  return counts;
}

/** Format a unix-millis timestamp as a short Norwegian date-time. */
export function formatTimestamp(ms: bigint): string {
  const d = new Date(Number(ms));
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleString("nb-NO", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
