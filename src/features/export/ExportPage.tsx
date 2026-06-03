/**
 * ExportPage — batch export (Phase 6).
 *
 * Delivers a real, CLAUDE.md core-promise feature: a church produces the SAME
 * Sunday program in several variants (regular, large-print, upload-ready) in one
 * pass. Layout mirrors Builder/Editor: a left panel (project + a multi-select
 * document checklist), a center options panel (paper size, large-print scaling,
 * output folder), and a right preview of the first selected document.
 *
 * The preview reuses the exact render→compile chain the Builder/Editor run
 * (`bulletin.render` → `bulletin.typstCompile`). The export itself drives the new
 * `exporter.batch` command, which loops that same chain over every selected
 * document on the backend and writes the PDFs to the chosen folder.
 */

import { useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckCircle2,
  FileText,
  FolderOpen,
  Loader2,
  Package,
  Sparkles,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { Document, ExportOptions } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { PdfPreview } from "@/features/builder/PdfPreview";

const PROJECTS_KEY = ["projects"] as const;
const documentsKey = (projectId: string) => ["documents", projectId] as const;

/** Paper sizes the options panel offers; "" means "keep each document's own". */
const PAPER_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "Behold dokumentets størrelse" },
  { value: "a4", label: "A4" },
  { value: "a5", label: "A5" },
  { value: "us-letter", label: "US Letter" },
];

/** Pull a readable message out of whatever a query/mutation rejected with. */
function errMessage(err: unknown, fallback: string): string {
  if (err instanceof IPCError) return `${err.code} — ${err.message}`;
  if (err instanceof Error) return err.message;
  return fallback;
}

export function ExportPage() {
  const [projectId, setProjectId] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [paper, setPaper] = useState("");
  const [largePrint, setLargePrint] = useState(false);
  const [largePrintPercent, setLargePrintPercent] = useState(150);
  const [outDir, setOutDir] = useState("");
  const [previewBase64, setPreviewBase64] = useState<string | null>(null);

  // ── Data ────────────────────────────────────────────────────────────────────
  const projects = useQuery({
    queryKey: PROJECTS_KEY,
    queryFn: () => ipc.project.list(),
  });

  const documents = useQuery({
    queryKey: documentsKey(projectId),
    queryFn: () => ipc.document.list(projectId),
    enabled: !!projectId,
  });

  const docList: Document[] = documents.data ?? [];

  const onSelectProject = (id: string) => {
    setProjectId(id);
    setSelected(new Set()); // a different project's documents differ
    setPreviewBase64(null);
  };

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setPreviewBase64(null);
  };

  // The export options the user assembled — `null` where "leave default".
  const options: ExportOptions = useMemo(
    () => ({
      paper: paper || null,
      largePrintPercent: largePrint ? largePrintPercent : null,
      lang: null,
    }),
    [paper, largePrint, largePrintPercent],
  );

  // First selected doc, in the order they appear in the list — that's what the
  // preview renders so it matches the first file the batch will write.
  const firstSelectedId = docList.find((d) => selected.has(d.id))?.id ?? null;

  // ── Preview: render→compile the first selected document ──────────────────────
  const preview = useMutation({
    mutationFn: async (docId: string) => {
      const source = await ipc.bulletin.render(docId);
      return ipc.bulletin.typstCompile(source);
    },
    onSuccess: (base64) => setPreviewBase64(base64),
  });

  // ── Export: the batch command ─────────────────────────────────────────────────
  const exportBatch = useMutation({
    mutationFn: () => {
      const ids = docList.filter((d) => selected.has(d.id)).map((d) => d.id);
      if (ids.length === 0) throw new Error("Velg minst ett dokument.");
      if (!outDir.trim()) throw new Error("Velg en målmappe.");
      return ipc.exporter.batch(ids, options, outDir.trim());
    },
  });

  const canExport =
    selected.size > 0 && !!outDir.trim() && !exportBatch.isPending;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Left: project + multi-select documents */}
      <div className="flex w-[340px] shrink-0 flex-col overflow-hidden border-r border-[var(--color-border)]">
        <header className="border-b border-[var(--color-border)] px-6 py-4">
          <div className="text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 6 · Eksport
          </div>
          <h1 className="mt-0.5 text-[var(--text-ui-xl)] font-bold">
            Samleeksport
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Eksporter flere dokumenter til PDF i én operasjon.
          </p>
        </header>

        <div className="flex-1 space-y-6 overflow-y-auto p-6">
          {/* Project picker */}
          <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              1 · Prosjekt
            </h2>
            {projects.isPending ? (
              <div className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
                <Loader2 size={14} className="animate-spin" />
                Laster prosjekter…
              </div>
            ) : projects.isError ? (
              <p className="text-sm text-[var(--color-danger)]">
                Kunne ikke laste prosjekter:{" "}
                {errMessage(projects.error, "ukjent feil")}
              </p>
            ) : (projects.data?.length ?? 0) > 0 ? (
              <select
                aria-label="Velg prosjekt"
                value={projectId}
                onChange={(e) => onSelectProject(e.target.value)}
                className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm"
              >
                <option value="">Velg prosjekt…</option>
                {projects.data!.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            ) : (
              <p className="text-sm text-[var(--color-fg-muted)]">
                Ingen prosjekter ennå — lag ett i Byggeren.
              </p>
            )}
          </section>

          {/* Document multi-select */}
          <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              2 · Dokumenter{" "}
              {selected.size > 0 && (
                <span className="text-[var(--color-accent)]">
                  ({selected.size} valgt)
                </span>
              )}
            </h2>
            {!projectId ? (
              <p className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
                <FolderOpen size={14} className="opacity-60" />
                Velg et prosjekt først.
              </p>
            ) : documents.isPending ? (
              <div className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
                <Loader2 size={14} className="animate-spin" />
                Laster dokumenter…
              </div>
            ) : documents.isError ? (
              <p className="text-sm text-[var(--color-danger)]">
                Kunne ikke laste dokumenter:{" "}
                {errMessage(documents.error, "ukjent feil")}
              </p>
            ) : docList.length > 0 ? (
              <ul className="space-y-1">
                {docList.map((d) => {
                  const isChecked = selected.has(d.id);
                  return (
                    <li key={d.id}>
                      <label
                        className={cn(
                          "flex w-full cursor-pointer items-center gap-2 rounded-md border px-2.5 py-2 text-left text-sm transition-colors",
                          isChecked
                            ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_10%,transparent)] text-[var(--color-fg)]"
                            : "border-[var(--color-border)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
                        )}
                      >
                        <input
                          type="checkbox"
                          aria-label={`Velg ${d.title}`}
                          checked={isChecked}
                          onChange={() => toggle(d.id)}
                          className="accent-[var(--color-accent)]"
                        />
                        <FileText
                          size={14}
                          className={
                            isChecked ? "text-[var(--color-accent)]" : undefined
                          }
                        />
                        <span className="min-w-0 flex-1 truncate font-medium">
                          {d.title}
                        </span>
                        <span className="shrink-0 text-xs text-[var(--color-fg-muted)]">
                          {d.kind}
                        </span>
                      </label>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <p className="text-sm text-[var(--color-fg-muted)]">
                Dette prosjektet har ingen dokumenter ennå.
              </p>
            )}
          </section>
        </div>
      </div>

      {/* Center: options */}
      <div className="flex min-w-0 flex-1 flex-col overflow-y-auto border-r border-[var(--color-border)] p-6">
        <h2 className="mb-4 text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
          3 · Eksportvalg
        </h2>

        <div className="space-y-5">
          {/* Paper */}
          <div className="space-y-1.5">
            <label
              htmlFor="export-paper"
              className="text-sm font-medium text-[var(--color-fg)]"
            >
              Papirstørrelse
            </label>
            <select
              id="export-paper"
              aria-label="Papirstørrelse"
              value={paper}
              onChange={(e) => setPaper(e.target.value)}
              className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm"
            >
              {PAPER_OPTIONS.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>

          {/* Large print */}
          <div className="space-y-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3">
            <label className="flex items-center gap-2 text-sm font-medium">
              <input
                type="checkbox"
                aria-label="Storskrift"
                checked={largePrint}
                onChange={(e) => setLargePrint(e.target.checked)}
                className="accent-[var(--color-accent)]"
              />
              Storskrift (forstørret tekst)
            </label>
            {largePrint && (
              <div className="flex items-center gap-3 pl-6">
                <input
                  type="range"
                  aria-label="Skalering i prosent"
                  min={100}
                  max={300}
                  step={10}
                  value={largePrintPercent}
                  onChange={(e) => setLargePrintPercent(Number(e.target.value))}
                  className="flex-1 accent-[var(--color-accent)]"
                />
                <span className="w-12 shrink-0 text-right text-sm font-semibold tabular-nums">
                  {largePrintPercent}%
                </span>
              </div>
            )}
          </div>

          {/* Output folder */}
          <div className="space-y-1.5">
            <label
              htmlFor="export-outdir"
              className="text-sm font-medium text-[var(--color-fg)]"
            >
              Målmappe
            </label>
            <input
              id="export-outdir"
              aria-label="Målmappe"
              value={outDir}
              placeholder="/Users/…/Skrivebord/program"
              onChange={(e) => setOutDir(e.target.value)}
              className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm"
            />
            <p className="text-xs text-[var(--color-fg-muted)]">
              Mappen opprettes om den ikke finnes.
            </p>
          </div>

          {/* Export button + result */}
          <div className="space-y-2 pt-1">
            {exportBatch.isError && (
              <ErrorBanner
                message={errMessage(exportBatch.error, "Kunne ikke eksportere")}
              />
            )}

            <button
              type="button"
              onClick={() => exportBatch.mutate()}
              disabled={!canExport}
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-[var(--color-accent)] px-3 py-2.5 text-sm font-bold text-[var(--color-accent-fg)] shadow-sm transition-all hover:brightness-110 active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50"
            >
              {exportBatch.isPending ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Package size={16} />
              )}
              Eksporter {selected.size > 0 ? `(${selected.size})` : ""}
            </button>

            {exportBatch.data && (
              <div className="space-y-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3">
                <div className="flex items-center gap-2 text-sm font-medium text-[var(--color-fg)]">
                  <CheckCircle2
                    size={15}
                    className="text-[var(--color-accent)]"
                  />
                  {exportBatch.data.files.length} fil(er) eksportert
                </div>
                <p className="truncate text-xs text-[var(--color-fg-muted)]">
                  {exportBatch.data.directory}
                </p>
                <ul className="space-y-0.5">
                  {exportBatch.data.files.map((f) => (
                    <li
                      key={f.documentId}
                      className="truncate text-xs text-[var(--color-fg-muted)]"
                    >
                      {f.fileName}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right: preview of the first selected document */}
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden p-6">
        {firstSelectedId && (
          <div className="pb-3">
            {preview.isError && (
              <div className="pb-2">
                <ErrorBanner
                  message={errMessage(
                    preview.error,
                    "Kunne ikke kompilere forhåndsvisning",
                  )}
                />
              </div>
            )}
            <button
              type="button"
              onClick={() => preview.mutate(firstSelectedId)}
              disabled={preview.isPending}
              className="flex w-full items-center justify-center gap-2 rounded-lg border border-[var(--color-accent)] px-3 py-2 text-sm font-semibold text-[var(--color-accent)] transition-colors hover:bg-[color-mix(in_oklch,var(--color-accent)_10%,transparent)] disabled:opacity-50"
            >
              {preview.isPending ? (
                <Loader2 size={15} className="animate-spin" />
              ) : (
                <Sparkles size={15} />
              )}
              Forhåndsvis første
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1">
          {previewBase64 ? (
            <PdfPreview base64={previewBase64} fileName="forhandsvisning.pdf" />
          ) : (
            <div className="grid h-full place-items-center rounded-xl border border-dashed border-[var(--color-border)]">
              <div className="max-w-xs text-center">
                <Sparkles
                  size={40}
                  className="mx-auto mb-3 text-[var(--color-fg-muted)] opacity-40"
                />
                <p className="text-sm text-[var(--color-fg-muted)]">
                  {firstSelectedId
                    ? "Trykk «Forhåndsvis første» for å se det første dokumentet."
                    : "Velg dokumenter til venstre for å eksportere dem."}
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── ErrorBanner ───────────────────────────────────────────────────────────────

function ErrorBanner({ message }: { message: string }) {
  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-3 py-2 text-sm text-[var(--color-danger)]"
    >
      <AlertCircle size={14} className="mt-0.5 shrink-0" />
      <span>{message}</span>
    </div>
  );
}
