/**
 * Projects panel — the first feature wired end to end through the Phase 1 data
 * layer. It lists / creates / deletes projects via `ipc.project`, so a packaged
 * testing build exercises the whole stack: IPC → command → repo → sqlx →
 * migrated SQLite on disk. If this works in the installed app, the data layer
 * is proven natively, not just in unit tests.
 */
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderPlus, Loader2, Trash2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";

const KEY = ["projects"];

export function ProjectsPanel() {
  const qc = useQueryClient();
  const [name, setName] = useState("");

  const projects = useQuery({
    queryKey: KEY,
    queryFn: () => ipc.project.list(),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: KEY });

  const create = useMutation({
    mutationFn: (n: string) => ipc.project.create(n),
    onSuccess: () => {
      setName("");
      invalidate();
    },
  });

  const remove = useMutation({
    mutationFn: (id: string) => ipc.project.delete(id),
    onSuccess: invalidate,
  });

  const trimmed = name.trim();

  return (
    <div className="mt-6 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-5 shadow-[var(--shadow-soft)]">
      <h2 className="mb-3 text-sm font-semibold">Prosjekter</h2>

      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (trimmed) create.mutate(trimmed);
        }}
      >
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nytt prosjekt …"
          className="flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-sm outline-none focus:border-[var(--color-accent)]"
        />
        <button
          type="submit"
          disabled={!trimmed || create.isPending}
          className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
        >
          <FolderPlus size={14} />
          Opprett
        </button>
      </form>

      {create.isError && (
        <p className="mt-2 text-xs text-[var(--color-danger)]">
          {create.error instanceof IPCError
            ? create.error.message
            : "Kunne ikke opprette prosjekt"}
        </p>
      )}

      <div className="mt-4">
        {projects.isPending ? (
          <p className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
            <Loader2 size={14} className="animate-spin" /> Laster …
          </p>
        ) : projects.isError ? (
          <p className="text-sm text-[var(--color-danger)]">
            Kunne ikke laste prosjekter:{" "}
            {projects.error instanceof IPCError
              ? `${projects.error.code} — ${projects.error.message}`
              : String(projects.error)}
          </p>
        ) : projects.data.length === 0 ? (
          <p className="text-sm text-[var(--color-fg-muted)]">
            Ingen prosjekter ennå. Opprett ett over for å teste datalaget.
          </p>
        ) : (
          <ul className="divide-y divide-[var(--color-border)]">
            {projects.data.map((p) => (
              <li
                key={p.id}
                className="flex items-center justify-between py-2 text-sm"
              >
                <span className="truncate">{p.name}</span>
                <button
                  type="button"
                  aria-label={`Slett ${p.name}`}
                  disabled={remove.isPending}
                  onClick={() => remove.mutate(p.id)}
                  className="rounded-md p-1 text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-danger)] disabled:opacity-50"
                >
                  <Trash2 size={14} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
