/**
 * Sangbok-klipper panel — lets the user upload a scanned hymnal PDF and
 * monitors the OCR pipeline job.
 *
 * Architecture:
 *   - `sangbok_import(pdf_path)` queues + stubs an OCR job
 *   - `sangbok_list_jobs()` polls job list (manual refresh button + on-mount)
 *   - `sangbok_get_job(id)` fetches detail
 *   - `sangbok_cancel(id)` cancels a non-terminal job
 *
 * OCR is currently a stub: jobs immediately reach Done with an empty song list
 * and a "not_implemented" detail. The UI makes this clear with a badge.
 */

import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Book,
  FileSearch,
  Loader2,
  Music,
  RefreshCcw,
  Scissors,
  Upload,
  X,
  AlertCircle,
  CheckCircle2,
  Clock,
  XCircle,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { SangbokJob, SongExtract } from "@/lib/bindings";
import { cn } from "@/lib/cn";

// ── Constants ─────────────────────────────────────────────────────────────────

const JOBS_QUERY_KEY = ["sangbok", "jobs"] as const;

// ── Status helpers ────────────────────────────────────────────────────────────

type Status = "Queued" | "Processing" | "Done" | "Failed";

function StatusBadge({ status }: { status: string }) {
  const map: Record<
    Status,
    { label: string; icon: typeof Clock; color: string }
  > = {
    Queued: {
      label: "Venter",
      icon: Clock,
      color:
        "text-[oklch(0.7_0.14_60)] bg-[color-mix(in_oklch,oklch(0.7_0.14_60)_12%,transparent)]",
    },
    Processing: {
      label: "Behandler",
      icon: Loader2,
      color:
        "text-[oklch(0.7_0.16_260)] bg-[color-mix(in_oklch,oklch(0.7_0.16_260)_12%,transparent)]",
    },
    Done: {
      label: "Ferdig",
      icon: CheckCircle2,
      color:
        "text-[oklch(0.7_0.18_145)] bg-[color-mix(in_oklch,oklch(0.7_0.18_145)_12%,transparent)]",
    },
    Failed: {
      label: "Feil",
      icon: XCircle,
      color:
        "text-[var(--color-danger)] bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)]",
    },
  };

  const s = map[status as Status] ?? map.Failed;
  const Icon = s.icon;

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold",
        s.color,
      )}
    >
      <Icon
        size={11}
        className={status === "Processing" ? "animate-spin" : ""}
      />
      {s.label}
    </span>
  );
}

// ── SongExtractRow ────────────────────────────────────────────────────────────

function SongExtractRow({ extract }: { extract: SongExtract }) {
  const pct = Math.round(extract.confidence * 100);
  return (
    <div className="flex items-center gap-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-2">
      <Music size={14} className="shrink-0 text-[var(--color-accent)]" />
      <div className="flex-1 min-w-0">
        <p className="truncate text-sm font-medium">
          {extract.title_hint || "(uten tittel)"}
        </p>
        <p className="text-xs text-[var(--color-fg-muted)]">
          Side {extract.page_start + 1}–{extract.page_end + 1}
        </p>
      </div>
      <div className="shrink-0 text-xs text-[var(--color-fg-muted)]">
        {pct}% konf.
      </div>
    </div>
  );
}

// ── JobCard ───────────────────────────────────────────────────────────────────

function JobCard({
  job,
  onCancel,
  isCancelling,
}: {
  job: SangbokJob;
  onCancel: (id: string) => void;
  isCancelling: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const isStub = job.error_detail?.includes("not_implemented") ?? false;
  const canCancel = job.status === "Queued" || job.status === "Processing";

  return (
    <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-[var(--shadow-soft)]">
      {/* Header */}
      <div className="flex items-start gap-3 p-4">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[color-mix(in_oklch,var(--color-accent)_12%,transparent)] text-[var(--color-accent)]">
          <Book size={16} />
        </div>

        <div className="flex-1 min-w-0">
          <p
            className="truncate text-sm font-semibold"
            title={job.input_pdf_path}
          >
            {job.input_pdf_path.split("/").pop() ?? job.input_pdf_path}
          </p>
          <p className="mt-0.5 truncate text-xs text-[var(--color-fg-muted)]">
            {job.input_pdf_path}
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <StatusBadge status={job.status} />
            {job.page_count > 0 && (
              <span className="text-xs text-[var(--color-fg-muted)]">
                {job.page_count} sider
              </span>
            )}
            {job.songs_found.length > 0 && (
              <span className="text-xs text-[var(--color-fg-muted)]">
                {job.songs_found.length} sanger funnet
              </span>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 items-center gap-1.5">
          {job.status === "Done" && job.songs_found.length > 0 && (
            <button
              type="button"
              onClick={() => setExpanded((e) => !e)}
              className="rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs font-medium text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
            >
              {expanded ? "Skjul" : "Vis sanger"}
            </button>
          )}
          {canCancel && (
            <button
              type="button"
              disabled={isCancelling}
              aria-label="Avbryt jobb"
              onClick={() => onCancel(job.id)}
              className="flex items-center justify-center rounded-md p-1.5 text-[var(--color-fg-muted)] transition-colors hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-danger)] disabled:opacity-50"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Stub notice */}
      {isStub && job.status === "Done" && (
        <div className="mx-4 mb-3 flex items-start gap-2 rounded-lg border border-dashed border-[oklch(0.7_0.14_60)] bg-[color-mix(in_oklch,oklch(0.7_0.14_60)_8%,transparent)] px-3 py-2 text-xs text-[oklch(0.65_0.14_60)]">
          <AlertCircle size={13} className="mt-0.5 shrink-0" />
          <span>
            <strong>OCR ikke aktivert.</strong> Reell OCR krever Tesseract og
            den <code>ocr</code> cargo-featuren. Jobb fullført som stub uten
            sange.
          </span>
        </div>
      )}

      {/* Error detail (non-stub failures) */}
      {job.status === "Failed" && !isStub && job.error_detail && (
        <div className="mx-4 mb-3 rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_8%,transparent)] px-3 py-2 text-xs text-[var(--color-danger)]">
          {job.error_detail}
        </div>
      )}

      {/* Expanded extract list */}
      {expanded && job.songs_found.length > 0 && (
        <div className="border-t border-[var(--color-border)] p-4">
          <p className="mb-2 text-xs font-semibold text-[var(--color-fg-muted)] uppercase tracking-wider">
            Funnet sanger
          </p>
          <div className="flex flex-col gap-1.5">
            {job.songs_found.map((ex) => (
              <SongExtractRow key={ex.id} extract={ex} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Upload zone ───────────────────────────────────────────────────────────────

function UploadZone({
  isDragOver,
  onDragOver,
  onDragLeave,
  onDrop,
  onFiles,
  isPending,
}: {
  isDragOver: boolean;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent) => void;
  onFiles: (files: FileList) => void;
  isPending: boolean;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed py-12 text-center transition-colors",
        isDragOver
          ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_8%,transparent)]"
          : "border-[var(--color-border)] hover:border-[var(--color-fg-muted)]",
      )}
    >
      {isPending ? (
        <Loader2
          size={32}
          className="animate-spin text-[var(--color-accent)]"
        />
      ) : (
        <Scissors
          size={32}
          className={
            isDragOver
              ? "text-[var(--color-accent)]"
              : "text-[var(--color-fg-muted)]"
          }
          aria-hidden
        />
      )}

      <div>
        <p className="text-sm font-medium">
          {isDragOver
            ? "Slipp PDF-filen her"
            : isPending
              ? "Behandler…"
              : "Dra en sangbok-PDF hit"}
        </p>
        <p className="mt-1 text-xs text-[var(--color-fg-muted)]">
          Støttede formater: PDF. OCR kjøres automatisk (stub i denne
          versjonen).
        </p>
      </div>

      {!isPending && (
        <button
          type="button"
          onClick={() => inputRef.current?.click()}
          className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-xs font-medium text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] transition-colors"
        >
          <Upload size={12} />
          Velg PDF-fil
        </button>
      )}

      <input
        ref={inputRef}
        type="file"
        accept=".pdf,application/pdf"
        className="sr-only"
        onChange={(e) => {
          if (e.target.files?.length) {
            onFiles(e.target.files);
            e.target.value = "";
          }
        }}
      />
    </div>
  );
}

// ── SangbokPanel ──────────────────────────────────────────────────────────────

export function SangbokPanel() {
  const qc = useQueryClient();
  const [dragOver, setDragOver] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // ── Query ────────────────────────────────────────────────────────────────────

  const query = useQuery({
    queryKey: [...JOBS_QUERY_KEY],
    queryFn: () => ipc.sangbok.listJobs(),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: JOBS_QUERY_KEY });

  // ── Import mutation ──────────────────────────────────────────────────────────

  const importMutation = useMutation({
    mutationFn: (pdfPath: string) => ipc.sangbok.import(pdfPath),
    onSuccess: () => {
      setErrorMsg(null);
      invalidate();
    },
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke importere PDF",
      );
    },
  });

  // ── Cancel mutation ──────────────────────────────────────────────────────────

  const cancelMutation = useMutation({
    mutationFn: (id: string) => ipc.sangbok.cancel(id),
    onSuccess: invalidate,
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke avbryte jobb",
      );
    },
  });

  // ── Drag helpers ─────────────────────────────────────────────────────────────

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  };
  const handleDragLeave = () => setDragOver(false);
  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    processFiles(e.dataTransfer.files);
  };

  const processFiles = (files: FileList) => {
    const file = files[0];
    if (!file) return;
    const filePath = (file as unknown as { path?: string }).path ?? file.name;
    importMutation.mutate(filePath);
  };

  // ── Render ────────────────────────────────────────────────────────────────────

  const jobs: SangbokJob[] = query.data ?? [];

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
        <div>
          <h1 className="text-[var(--text-ui-xl)] font-bold">
            Sangbok-klipper
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Importer skannet sangbok-PDF og klipp ut enkelt-sanger (OCR stub i
            denne versjonen)
          </p>
        </div>
        <div className="flex items-center gap-2">
          {query.isFetching && (
            <Loader2
              size={14}
              className="animate-spin text-[var(--color-fg-muted)]"
            />
          )}
          <button
            type="button"
            aria-label="Oppdater liste"
            onClick={() => invalidate()}
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs font-medium text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] transition-colors"
          >
            <RefreshCcw size={12} />
            Oppdater
          </button>
        </div>
      </header>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 space-y-5">
        {/* Error banner */}
        {errorMsg && (
          <div className="flex items-start justify-between gap-3 rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-4 py-2.5 text-sm text-[var(--color-danger)]">
            <span>{errorMsg}</span>
            <button
              type="button"
              onClick={() => setErrorMsg(null)}
              className="shrink-0 text-xs underline"
            >
              Lukk
            </button>
          </div>
        )}

        {/* Upload zone */}
        <UploadZone
          isDragOver={dragOver}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onFiles={processFiles}
          isPending={importMutation.isPending}
        />

        {/* Job list */}
        {query.isError ? (
          <div className="flex flex-col items-center gap-3 py-10 text-center">
            <AlertCircle
              size={36}
              className="text-[var(--color-danger)] opacity-60"
            />
            <p className="text-sm text-[var(--color-danger)]">
              Kunne ikke laste jobber:{" "}
              {query.error instanceof IPCError
                ? `${query.error.code} — ${query.error.message}`
                : String(query.error)}
            </p>
          </div>
        ) : jobs.length === 0 && !query.isPending ? (
          <div className="flex flex-col items-center gap-3 py-10 text-center">
            <FileSearch
              size={36}
              className="text-[var(--color-fg-muted)] opacity-40"
            />
            <p className="text-sm text-[var(--color-fg-muted)]">
              Ingen importjobber ennå. Dra en sangbok-PDF hit for å starte.
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              Importjobber ({jobs.length})
            </h2>
            {jobs.map((job) => (
              <JobCard
                key={job.id}
                job={job}
                isCancelling={
                  cancelMutation.isPending &&
                  cancelMutation.variables === job.id
                }
                onCancel={(id) => cancelMutation.mutate(id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
