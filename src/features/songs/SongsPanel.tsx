/**
 * Song catalog panel — the first UI on top of the song CRUD IPC
 * (`ipc.song.*`). Lets a volunteer keep the parish song catalog: list every
 * song, compose a new one, edit an existing one, and delete with a confirm.
 *
 * Layout is master/detail: the catalog list on the left, an editor form on the
 * right. The Nordic reality (CLAUDE.md) is honoured by carrying an optional
 * `tono_work_id` so songs can later flow to SundaySong with their TONO id.
 *
 * Data flow:
 *   - Reads via `ipc.song.list()` (TanStack Query, keyed by `songsKey`)
 *   - Writes via `ipc.song.create` / `ipc.song.update` / `ipc.song.delete`
 *
 * Slots into the "songs" route in App.tsx.
 */

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Music, Plus, Save, Search, Trash2, X } from "lucide-react";

import { ipc, IPCError, errMessage } from "@/lib/ipc";
import { songsKey } from "@/lib/queryKeys";
import type { Song } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import {
  emptySongForm,
  formToPayload,
  isSongFormValid,
  songToForm,
  type SongFormState,
} from "./songForm";

/** A new, unsaved song is selected as the literal `"new"`; otherwise an id. */
type Selection = { kind: "new" } | { kind: "song"; id: string } | null;

export function SongsPanel() {
  const qc = useQueryClient();
  const [selection, setSelection] = useState<Selection>(null);
  const [form, setForm] = useState<SongFormState>(emptySongForm);
  const [search, setSearch] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const query = useQuery({
    queryKey: songsKey,
    queryFn: () => ipc.song.list(),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: songsKey });

  const create = useMutation({
    mutationFn: (f: SongFormState) => ipc.song.create(formToPayload(f)),
    onSuccess: (song) => {
      invalidate();
      setSelection({ kind: "song", id: song.id });
    },
  });

  const update = useMutation({
    mutationFn: ({ id, f }: { id: string; f: SongFormState }) =>
      ipc.song.update(id, formToPayload(f)),
    onSuccess: invalidate,
  });

  const remove = useMutation({
    mutationFn: (id: string) => ipc.song.delete(id),
    onSuccess: (_void, id) => {
      invalidate();
      setConfirmDelete(null);
      if (selection?.kind === "song" && selection.id === id) {
        setSelection(null);
      }
    },
  });

  const songs = useMemo(() => query.data ?? [], [query.data]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return songs;
    return songs.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        (s.author ?? "").toLowerCase().includes(q),
    );
  }, [songs, search]);

  /** Open a song (or the blank composer) in the editor pane. */
  function openSong(song: Song) {
    setSelection({ kind: "song", id: song.id });
    setForm(songToForm(song));
    create.reset();
    update.reset();
  }
  function openNew() {
    setSelection({ kind: "new" });
    setForm(emptySongForm);
    create.reset();
    update.reset();
  }
  function closeEditor() {
    setSelection(null);
    create.reset();
    update.reset();
  }

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isSongFormValid(form) || !selection) return;
    if (selection.kind === "new") {
      create.mutate(form);
    } else {
      update.mutate({ id: selection.id, f: form });
    }
  }

  const saving = create.isPending || update.isPending;
  const saveError = create.error ?? update.error;

  return (
    <div className="flex h-full overflow-hidden">
      {/* ── Master: catalog list ─────────────────────────────────────────── */}
      <section className="flex w-80 shrink-0 flex-col border-r border-[var(--color-border)]">
        <header className="flex items-center justify-between border-b border-[var(--color-border)] px-5 py-4">
          <div>
            <h1 className="text-[var(--text-ui-xl)] font-bold">Sangkatalog</h1>
            <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
              Menighetens sanger og salmer
            </p>
          </div>
          {query.isPending && (
            <Loader2
              size={16}
              className="animate-spin text-[var(--color-fg-muted)]"
            />
          )}
        </header>

        <div className="border-b border-[var(--color-border)] px-5 py-2.5">
          <div className="relative">
            <Search
              size={14}
              aria-hidden
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--color-fg-muted)]"
            />
            <input
              aria-label="Søk i sanger"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Søk på tittel eller forfatter …"
              className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] py-1.5 pl-8 pr-3 text-sm outline-none focus:border-[var(--color-accent)]"
            />
          </div>
          <button
            type="button"
            onClick={openNew}
            className="mt-2.5 flex w-full items-center justify-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-2 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110"
          >
            <Plus size={14} />
            Ny sang
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-2 py-2">
          {query.isError ? (
            <p className="px-3 py-2 text-sm text-[var(--color-danger)]">
              Kunne ikke laste sangkatalogen:{" "}
              {errMessage(query.error, "ukjent feil")}
            </p>
          ) : filtered.length === 0 ? (
            <p className="px-3 py-6 text-center text-sm text-[var(--color-fg-muted)]">
              {songs.length === 0
                ? "Ingen sanger ennå. Opprett en med «Ny sang»."
                : "Ingen treff for søket."}
            </p>
          ) : (
            <ul className="space-y-0.5">
              {filtered.map((s) => {
                const active =
                  selection?.kind === "song" && selection.id === s.id;
                return (
                  <li key={s.id}>
                    <button
                      type="button"
                      onClick={() => openSong(s)}
                      className={cn(
                        "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-left text-sm transition-colors",
                        active
                          ? "bg-[var(--color-bg-surface)] text-[var(--color-fg)]"
                          : "text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]/60 hover:text-[var(--color-fg)]",
                      )}
                    >
                      <Music size={14} aria-hidden className="shrink-0" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-medium">
                          {s.title}
                        </span>
                        {s.author && (
                          <span className="block truncate text-xs text-[var(--color-fg-muted)]">
                            {s.author}
                          </span>
                        )}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </section>

      {/* ── Detail: editor ───────────────────────────────────────────────── */}
      <section className="flex-1 overflow-y-auto">
        {!selection ? (
          <div className="grid h-full place-items-center">
            <div className="max-w-sm text-center text-[var(--color-fg-muted)]">
              <Music
                size={32}
                className="mx-auto mb-3 opacity-50"
                aria-hidden
              />
              <p className="text-sm">
                Velg en sang fra listen, eller opprett en ny for å redigere.
              </p>
            </div>
          </div>
        ) : (
          <form onSubmit={onSubmit} className="mx-auto max-w-2xl px-8 py-6">
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-[var(--text-ui-lg)] font-bold">
                {selection.kind === "new" ? "Ny sang" : "Rediger sang"}
              </h2>
              <button
                type="button"
                aria-label="Lukk redigering"
                onClick={closeEditor}
                className="rounded-md p-1.5 text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)]"
              >
                <X size={16} />
              </button>
            </div>

            <div className="space-y-4">
              <Field label="Tittel" required>
                <input
                  value={form.title}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, title: e.target.value }))
                  }
                  placeholder="Navn over alle navn"
                  className={inputCls}
                />
              </Field>

              <div className="grid grid-cols-2 gap-4">
                <Field label="Forfatter">
                  <input
                    value={form.author}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, author: e.target.value }))
                    }
                    placeholder="Forfatter / komponist"
                    className={inputCls}
                  />
                </Field>
                <Field label="Språk">
                  <input
                    value={form.language}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, language: e.target.value }))
                    }
                    placeholder="no, en, sv …"
                    className={inputCls}
                  />
                </Field>
              </div>

              <Field label="Tekst">
                <textarea
                  value={form.body}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, body: e.target.value }))
                  }
                  rows={12}
                  placeholder="Sangtekst …"
                  className={cn(inputCls, "resize-y font-mono")}
                />
              </Field>

              <Field label="TONO-verk-ID (valgfritt)">
                <input
                  value={form.tonoWorkId}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, tonoWorkId: e.target.value }))
                  }
                  placeholder="For SundaySong-overføring"
                  className={inputCls}
                />
              </Field>
            </div>

            {saveError && (
              <p className="mt-4 text-sm text-[var(--color-danger)]">
                {saveError instanceof IPCError
                  ? saveError.message
                  : "Kunne ikke lagre sangen"}
              </p>
            )}

            <div className="mt-6 flex items-center gap-3">
              <button
                type="submit"
                disabled={!isSongFormValid(form) || saving}
                className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
              >
                {saving ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Save size={14} />
                )}
                Lagre
              </button>

              {selection.kind === "song" &&
                (confirmDelete === selection.id ? (
                  <span className="flex items-center gap-2 text-sm">
                    <span className="text-[var(--color-fg-muted)]">
                      Slette for godt?
                    </span>
                    <button
                      type="button"
                      onClick={() => remove.mutate(selection.id)}
                      disabled={remove.isPending}
                      className="rounded-md bg-[var(--color-danger)] px-3 py-1.5 text-sm font-bold text-white hover:brightness-110 disabled:opacity-50"
                    >
                      Slett
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmDelete(null)}
                      className="rounded-md px-3 py-1.5 text-sm text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
                    >
                      Avbryt
                    </button>
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmDelete(selection.id)}
                    className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-fg-muted)] hover:border-[var(--color-danger)] hover:text-[var(--color-danger)]"
                  >
                    <Trash2 size={14} />
                    Slett
                  </button>
                ))}
            </div>

            {remove.isError && (
              <p className="mt-3 text-sm text-[var(--color-danger)]">
                {errMessage(remove.error, "Kunne ikke slette sangen")}
              </p>
            )}
          </form>
        )}
      </section>
    </div>
  );
}

const inputCls =
  "w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-2 text-sm outline-none focus:border-[var(--color-accent)]";

function Field({
  label,
  required,
  children,
}: {
  label: string;
  required?: boolean;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--color-fg-muted)]">
        {label}
        {required && <span className="text-[var(--color-danger)]"> *</span>}
      </span>
      {children}
    </label>
  );
}
