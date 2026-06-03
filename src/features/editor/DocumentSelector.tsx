/**
 * DocumentSelector — the left panel of the editor: pick a project, then a
 * document within it. Mirrors the project picker in BuilderPage but adds a
 * second level (documents) since the editor operates on an existing document
 * rather than generating a new one.
 *
 * Both lists come from `ipc.project.list()` / `ipc.document.list(projectId)`.
 * Selecting a project clears the document selection (its docs differ); the
 * parent owns the chosen ids so the rest of the editor can react.
 */

import { useQuery } from "@tanstack/react-query";
import { FileText, FolderOpen, Loader2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import { cn } from "@/lib/cn";

export const PROJECTS_KEY = ["projects"] as const;
/** Query key for a project's documents — exported so the editor can invalidate. */
export const documentsKey = (projectId: string) =>
  ["documents", projectId] as const;

/** Pull a readable message out of a query/mutation error. */
function errMessage(err: unknown, fallback: string): string {
  if (err instanceof IPCError) return `${err.code} — ${err.message}`;
  if (err instanceof Error) return err.message;
  return fallback;
}

interface DocumentSelectorProps {
  projectId: string;
  documentId: string;
  onSelectProject: (projectId: string) => void;
  onSelectDocument: (documentId: string) => void;
}

export function DocumentSelector({
  projectId,
  documentId,
  onSelectProject,
  onSelectDocument,
}: DocumentSelectorProps) {
  const projects = useQuery({
    queryKey: PROJECTS_KEY,
    queryFn: () => ipc.project.list(),
  });

  const documents = useQuery({
    queryKey: documentsKey(projectId),
    queryFn: () => ipc.document.list(projectId),
    enabled: !!projectId,
  });

  return (
    <div className="space-y-6">
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

      {/* Document picker — only once a project is chosen */}
      <section className="space-y-2">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
          2 · Dokument
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
        ) : (documents.data?.length ?? 0) > 0 ? (
          <ul className="space-y-1">
            {documents.data!.map((d) => {
              const isActive = d.id === documentId;
              return (
                <li key={d.id}>
                  <button
                    type="button"
                    aria-label={`Åpne ${d.title}`}
                    aria-pressed={isActive}
                    onClick={() => onSelectDocument(d.id)}
                    className={cn(
                      "flex w-full items-center gap-2 rounded-md border px-2.5 py-2 text-left text-sm transition-colors",
                      isActive
                        ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_10%,transparent)] text-[var(--color-fg)]"
                        : "border-[var(--color-border)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
                    )}
                  >
                    <FileText
                      size={14}
                      className={
                        isActive ? "text-[var(--color-accent)]" : undefined
                      }
                    />
                    <span className="min-w-0 flex-1 truncate font-medium">
                      {d.title}
                    </span>
                    <span className="shrink-0 text-xs text-[var(--color-fg-muted)]">
                      {d.kind}
                    </span>
                  </button>
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
  );
}
