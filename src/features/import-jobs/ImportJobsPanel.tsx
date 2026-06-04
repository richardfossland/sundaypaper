/**
 * Import-job history panel — a log of past ingest jobs (OCR, split, merge) on
 * top of `ipc.importJob.list()`. Shows each job's source file, kind, a
 * colour-coded status badge, timestamp, and any error detail.
 *
 * The log is now destructively editable: each row has a Delete (with confirm)
 * backed by `ipc.importJob.delete`, and a "Tøm ferdige" header action backed by
 * `ipc.importJob.clearFinished` removes every done/errored job in one DB op.
 *
 * Slots into the "imports" route in App.tsx.
 */

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  History,
  Loader2,
  RefreshCw,
  Trash2,
} from "lucide-react";

import { ipc, errMessage } from "@/lib/ipc";
import { importJobsKey } from "@/lib/queryKeys";
import { cn } from "@/lib/cn";
import {
  baseName,
  formatTimestamp,
  isFinished,
  kindLabel,
  normStatus,
  statusCounts,
  statusLabel,
  viewJobs,
  type JobStatus,
} from "./importJobView";

const STATUS_STYLE: Record<JobStatus, { cls: string; icon: typeof Clock }> = {
  pending: {
    cls: "border-[var(--color-border)] text-[var(--color-fg-muted)]",
    icon: Clock,
  },
  running: {
    cls: "border-[var(--color-accent)] text-[var(--color-accent)]",
    icon: Loader2,
  },
  done: {
    cls: "border-[oklch(0.6_0.14_145)] text-[oklch(0.6_0.14_145)]",
    icon: CheckCircle2,
  },
  error: {
    cls: "border-[var(--color-danger)] text-[var(--color-danger)]",
    icon: AlertCircle,
  },
};

function StatusBadge({ status }: { status: string }) {
  const s = normStatus(status);
  const { cls, icon: Icon } = STATUS_STYLE[s];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium",
        cls,
      )}
    >
      <Icon
        size={12}
        aria-hidden
        className={s === "running" ? "animate-spin" : undefined}
      />
      {statusLabel(status)}
    </span>
  );
}

export function ImportJobsPanel() {
  const qc = useQueryClient();
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const query = useQuery({
    queryKey: importJobsKey,
    queryFn: () => ipc.importJob.list(),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: importJobsKey });

  const remove = useMutation({
    mutationFn: (id: string) => ipc.importJob.delete(id),
    onSuccess: () => {
      invalidate();
      setConfirmDelete(null);
    },
  });

  const clearFinished = useMutation({
    mutationFn: () => ipc.importJob.clearFinished(),
    onSuccess: () => {
      invalidate();
      setConfirmClear(false);
    },
  });

  const jobs = useMemo(() => query.data ?? [], [query.data]);
  const counts = useMemo(() => statusCounts(jobs), [jobs]);
  // No client-side filtering any more — the list mirrors the DB exactly.
  const rows = useMemo(() => viewJobs(jobs, { hideFinished: false }), [jobs]);
  const hasFinished = useMemo(
    () => jobs.some((j) => isFinished(j.status)),
    [jobs],
  );

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
        <div>
          <h1 className="text-[var(--text-ui-xl)] font-bold">Importlogg</h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Tidligere OCR- og innlesingsjobber
          </p>
        </div>
        <div className="flex items-center gap-2">
          {hasFinished &&
            (confirmClear ? (
              <span className="flex items-center gap-2 text-xs">
                <span className="text-[var(--color-fg-muted)]">
                  Slette alle ferdige?
                </span>
                <button
                  type="button"
                  onClick={() => clearFinished.mutate()}
                  disabled={clearFinished.isPending}
                  className="rounded-md bg-[var(--color-danger)] px-2.5 py-1.5 font-bold text-white hover:brightness-110 disabled:opacity-50"
                >
                  Tøm
                </button>
                <button
                  type="button"
                  onClick={() => setConfirmClear(false)}
                  className="rounded-md px-2 py-1.5 text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
                >
                  Avbryt
                </button>
              </span>
            ) : (
              <button
                type="button"
                onClick={() => setConfirmClear(true)}
                className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-xs text-[var(--color-fg-muted)] hover:border-[var(--color-danger)] hover:text-[var(--color-danger)]"
              >
                <Trash2 size={13} />
                Tøm ferdige
              </button>
            ))}
          <button
            type="button"
            onClick={() => query.refetch()}
            disabled={query.isFetching}
            aria-label="Oppdater"
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-xs text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] disabled:opacity-50"
          >
            <RefreshCw
              size={13}
              className={query.isFetching ? "animate-spin" : undefined}
            />
            Oppdater
          </button>
        </div>
      </header>

      {clearFinished.isError && (
        <p
          role="alert"
          className="border-b border-[var(--color-border)] px-6 py-2 text-xs text-[var(--color-danger)]"
        >
          {errMessage(clearFinished.error, "Kunne ikke tømme ferdige jobber")}
        </p>
      )}

      {/* Summary counters */}
      {jobs.length > 0 && (
        <div className="flex flex-wrap gap-2 border-b border-[var(--color-border)] px-6 py-2.5 text-xs">
          {(["running", "pending", "done", "error"] as JobStatus[]).map((s) => (
            <span key={s} className="text-[var(--color-fg-muted)]">
              {statusLabel(s)}:{" "}
              <span className="font-semibold text-[var(--color-fg)]">
                {counts[s]}
              </span>
            </span>
          ))}
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-6 py-4">
        {query.isError ? (
          <p className="text-sm text-[var(--color-danger)]">
            Kunne ikke laste importloggen:{" "}
            {errMessage(query.error, "ukjent feil")}
          </p>
        ) : query.isPending ? (
          <p className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
            <Loader2 size={14} className="animate-spin" />
            Laster …
          </p>
        ) : rows.length === 0 ? (
          <div className="grid h-full place-items-center">
            <div className="max-w-sm text-center text-[var(--color-fg-muted)]">
              <History
                size={32}
                className="mx-auto mb-3 opacity-50"
                aria-hidden
              />
              <p className="text-sm">
                Ingen importjobber ennå. De vises her når du leser inn en PDF.
              </p>
            </div>
          </div>
        ) : (
          <ul className="space-y-2">
            {rows.map((job) => (
              <li
                key={job.id}
                className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium">
                        {baseName(job.source_path)}
                      </span>
                      <span className="shrink-0 rounded border border-[var(--color-border)] px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-[var(--color-fg-muted)]">
                        {kindLabel(job.kind)}
                      </span>
                    </div>
                    <div className="mt-0.5 truncate text-xs text-[var(--color-fg-muted)]">
                      {job.source_path}
                    </div>
                  </div>
                  <div className="flex shrink-0 flex-col items-end gap-1">
                    <StatusBadge status={job.status} />
                    <span className="text-[11px] text-[var(--color-fg-muted)]">
                      {formatTimestamp(job.created_at)}
                    </span>
                    {confirmDelete === job.id ? (
                      <span className="flex items-center gap-1.5 text-xs">
                        <button
                          type="button"
                          onClick={() => remove.mutate(job.id)}
                          disabled={remove.isPending}
                          className="rounded bg-[var(--color-danger)] px-2 py-0.5 font-bold text-white hover:brightness-110 disabled:opacity-50"
                        >
                          Slett
                        </button>
                        <button
                          type="button"
                          onClick={() => setConfirmDelete(null)}
                          className="rounded px-1.5 py-0.5 text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
                        >
                          Avbryt
                        </button>
                      </span>
                    ) : (
                      <button
                        type="button"
                        onClick={() => setConfirmDelete(job.id)}
                        aria-label={`Slett ${baseName(job.source_path)}`}
                        className="rounded p-1 text-[var(--color-fg-muted)] hover:text-[var(--color-danger)]"
                      >
                        <Trash2 size={13} />
                      </button>
                    )}
                  </div>
                </div>

                {job.detail && normStatus(job.status) === "error" && (
                  <p
                    role="alert"
                    className="mt-2 rounded-md bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-2.5 py-1.5 text-xs text-[var(--color-danger)]"
                  >
                    {job.detail}
                  </p>
                )}
                {job.detail && normStatus(job.status) !== "error" && (
                  <p className="mt-2 text-xs text-[var(--color-fg-muted)]">
                    {job.detail}
                  </p>
                )}
                {remove.isError && confirmDelete === job.id && (
                  <p
                    role="alert"
                    className="mt-2 text-xs text-[var(--color-danger)]"
                  >
                    {errMessage(remove.error, "Kunne ikke slette jobben")}
                  </p>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
