/**
 * Asset Library panel — the heart of the "backward" direction.
 *
 * Displays all library assets in a responsive grid. Each card shows an icon
 * per asset type, the name, comma-separated tags, and controls to open or
 * delete the asset. A drag-target overlay lets the user register new assets by
 * dropping files directly onto the panel.
 *
 * Data flow:
 *   - Reads via `ipc.assetLib.list(kindFilter)` (TanStack Query)
 *   - Writes via `ipc.assetLib.add` / `ipc.assetLib.delete` (mutations)
 *   - Opens  via `ipc.assetLib.open`  (fire-and-forget)
 *
 * The panel is designed to slot into the "library" route in App.tsx.
 */

import { useState, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ImageIcon,
  LayoutTemplate,
  Music,
  RefreshCcw,
  Type,
  Trash2,
  ExternalLink,
  Upload,
  Loader2,
  FolderOpen,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { AssetKind, AssetLibEntry } from "@/lib/bindings";
import { cn } from "@/lib/cn";

// ── Constants ─────────────────────────────────────────────────────────────────

const KIND_LABELS: Record<AssetKind, string> = {
  Logo: "Logo",
  Template: "Mal",
  SongSheet: "Sangarket",
  RecurringBlock: "Gjenbruksblokk",
  Font: "Skrifttype",
};

const FILTER_OPTIONS: Array<{ value: AssetKind | "all"; label: string }> = [
  { value: "all", label: "Alle" },
  { value: "Logo", label: "Logoer" },
  { value: "Template", label: "Maler" },
  { value: "SongSheet", label: "Sangark" },
  { value: "RecurringBlock", label: "Gjenbruk" },
  { value: "Font", label: "Skrifttyper" },
];

// ── Icon per kind ─────────────────────────────────────────────────────────────

function KindIcon({ kind, size = 20 }: { kind: AssetKind; size?: number }) {
  const props = { size, "aria-hidden": true };
  switch (kind) {
    case "Logo":
      return <ImageIcon {...props} />;
    case "Template":
      return <LayoutTemplate {...props} />;
    case "SongSheet":
      return <Music {...props} />;
    case "RecurringBlock":
      return <RefreshCcw {...props} />;
    case "Font":
      return <Type {...props} />;
  }
}

const KIND_ACCENT: Record<AssetKind, string> = {
  Logo: "oklch(0.76 0.14 52)", // copper — brand accent
  Template: "oklch(0.7 0.16 280)", // indigo
  SongSheet: "oklch(0.74 0.18 145)", // green
  RecurringBlock: "oklch(0.8 0.16 75)", // amber
  Font: "oklch(0.7 0.14 245)", // blue
};

// ── Asset card ────────────────────────────────────────────────────────────────

function AssetCard({
  entry,
  onDelete,
  onOpen,
  isDeleting,
}: {
  entry: AssetLibEntry;
  onDelete: (id: string) => void;
  onOpen: (id: string) => void;
  isDeleting: boolean;
}) {
  const tags = entry.tags
    ? entry.tags
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean)
    : [];
  const accent = KIND_ACCENT[entry.kind];

  return (
    <div className="group relative flex flex-col gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4 shadow-[var(--shadow-soft)] transition-shadow hover:shadow-[var(--shadow-popover)]">
      {/* Kind icon */}
      <div
        className="flex h-10 w-10 items-center justify-center rounded-lg"
        style={{
          background: `color-mix(in oklch, ${accent} 15%, transparent)`,
          color: accent,
        }}
      >
        <KindIcon kind={entry.kind} size={18} />
      </div>

      {/* Name + kind badge */}
      <div className="flex-1">
        <p className="truncate text-sm font-semibold leading-tight">
          {entry.name}
        </p>
        <p className="mt-0.5 text-xs font-medium" style={{ color: accent }}>
          {KIND_LABELS[entry.kind]}
        </p>
      </div>

      {/* Tags */}
      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {tags.map((tag) => (
            <span
              key={tag}
              className="rounded-full border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2 py-0.5 text-[10px] text-[var(--color-fg-muted)]"
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Actions — revealed on hover */}
      <div className="flex gap-1.5">
        <button
          type="button"
          aria-label={`Åpne ${entry.name}`}
          onClick={() => onOpen(entry.id)}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-[var(--color-bg-surface)] py-1.5 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:bg-[var(--color-border)] hover:text-[var(--color-fg)]"
        >
          <ExternalLink size={12} />
          Åpne
        </button>
        <button
          type="button"
          aria-label={`Slett ${entry.name}`}
          disabled={isDeleting}
          onClick={() => onDelete(entry.id)}
          className="flex items-center justify-center rounded-md p-1.5 text-[var(--color-fg-muted)] transition-colors hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-danger)] disabled:opacity-50"
        >
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}

// ── Drop zone ─────────────────────────────────────────────────────────────────

function DropZone({
  onFiles,
  isDragOver,
  onDragOver,
  onDragLeave,
  onDrop,
}: {
  onFiles: (files: FileList) => void;
  isDragOver: boolean;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed py-10 text-center transition-colors",
        isDragOver
          ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_8%,transparent)]"
          : "border-[var(--color-border)] bg-transparent hover:border-[var(--color-fg-muted)]",
      )}
    >
      <Upload
        size={28}
        className={
          isDragOver
            ? "text-[var(--color-accent)]"
            : "text-[var(--color-fg-muted)]"
        }
        aria-hidden
      />
      <div>
        <p className="text-sm font-medium">
          {isDragOver ? "Slipp filen her" : "Dra filer hit for å legge til"}
        </p>
        <p className="mt-1 text-xs text-[var(--color-fg-muted)]">
          Logo, mal, sangark, skrifttype …
        </p>
      </div>
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-xs font-medium text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] transition-colors"
      >
        Velg filer
      </button>
      <input
        ref={inputRef}
        type="file"
        multiple
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

// ── Kind guesser ──────────────────────────────────────────────────────────────

/** Heuristic: guess the AssetKind from a filename extension. */
function guessKind(filename: string): AssetKind {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  if (["ttf", "otf", "woff", "woff2"].includes(ext)) return "Font";
  if (["png", "svg", "jpg", "jpeg", "gif", "webp"].includes(ext)) return "Logo";
  if (["typ"].includes(ext)) return "Template";
  if (["pdf"].includes(ext)) return "SongSheet";
  return "Logo";
}

// ── KindSelector ──────────────────────────────────────────────────────────────

function KindSelector({
  value,
  onChange,
  label,
}: {
  value: AssetKind;
  onChange: (k: AssetKind) => void;
  label: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs text-[var(--color-fg-muted)]">{label}</label>
      <div className="flex flex-wrap gap-1.5">
        {(
          [
            "Logo",
            "Template",
            "SongSheet",
            "RecurringBlock",
            "Font",
          ] as AssetKind[]
        ).map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => onChange(k)}
            className={cn(
              "flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium transition-colors",
              value === k
                ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_15%,transparent)] text-[var(--color-accent)]"
                : "border-[var(--color-border)] text-[var(--color-fg-muted)] hover:border-[var(--color-fg-muted)]",
            )}
          >
            <KindIcon kind={k} size={11} />
            {KIND_LABELS[k]}
          </button>
        ))}
      </div>
    </div>
  );
}

// ── AddDialog (inline quick-add) ──────────────────────────────────────────────

function AddForm({
  fileName,
  filePath,
  defaultKind,
  onSubmit,
  onCancel,
  isPending,
}: {
  fileName: string;
  filePath: string;
  defaultKind: AssetKind;
  onSubmit: (input: { name: string; kind: AssetKind; tags: string }) => void;
  onCancel: () => void;
  isPending: boolean;
}) {
  const [name, setName] = useState(fileName);
  const [kind, setKind] = useState<AssetKind>(defaultKind);
  const [tags, setTags] = useState("");

  return (
    <form
      className="flex flex-col gap-3 rounded-xl border border-[var(--color-accent)] bg-[var(--color-bg-elevated)] p-4 shadow-[var(--shadow-popover)]"
      onSubmit={(e) => {
        e.preventDefault();
        const trimmed = name.trim();
        if (trimmed) onSubmit({ name: trimmed, kind, tags: tags.trim() });
      }}
    >
      <p className="truncate text-xs text-[var(--color-fg-muted)]">
        {filePath}
      </p>

      <input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Navn på asset …"
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-sm outline-none focus:border-[var(--color-accent)]"
      />

      <KindSelector value={kind} onChange={setKind} label="Type" />

      <input
        value={tags}
        onChange={(e) => setTags(e.target.value)}
        placeholder="Tagger (komma-separert) …"
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-xs outline-none focus:border-[var(--color-accent)]"
      />

      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!name.trim() || isPending}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-[var(--color-accent)] py-1.5 text-xs font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
        >
          {isPending ? <Loader2 size={12} className="animate-spin" /> : null}
          Legg til
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
        >
          Avbryt
        </button>
      </div>
    </form>
  );
}

// ── PendingFile shape ─────────────────────────────────────────────────────────

interface PendingFile {
  name: string;
  path: string;
  kind: AssetKind;
}

// ── AssetsPanel ───────────────────────────────────────────────────────────────

const QUERY_KEY = ["assets", "lib"] as const;

export function AssetsPanel() {
  const qc = useQueryClient();
  const [kindFilter, setKindFilter] = useState<AssetKind | "all">("all");
  const [dragOver, setDragOver] = useState(false);
  const [pending, setPending] = useState<PendingFile | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // ── Query ──────────────────────────────────────────────────────────────────

  const query = useQuery({
    queryKey: [...QUERY_KEY, kindFilter],
    queryFn: () =>
      ipc.assetLib.list(kindFilter === "all" ? undefined : kindFilter),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: QUERY_KEY });

  // ── Mutations ──────────────────────────────────────────────────────────────

  const addMutation = useMutation({
    mutationFn: (input: {
      name: string;
      kind: AssetKind;
      filePath: string;
      tags?: string;
    }) => ipc.assetLib.add(input),
    onSuccess: () => {
      setPending(null);
      setErrorMsg(null);
      invalidate();
    },
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke legge til asset",
      );
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => ipc.assetLib.delete(id),
    onSuccess: invalidate,
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke slette asset",
      );
    },
  });

  const openMutation = useMutation({
    mutationFn: (id: string) => ipc.assetLib.open(id),
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke åpne filen",
      );
    },
  });

  // ── Drag helpers ───────────────────────────────────────────────────────────

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
    const first = files[0];
    if (!first) return;
    // In Tauri a dropped File object's `.path` (non-standard) carries the
    // absolute path on macOS/Windows. Fall back to name for web builds.
    const filePath = (first as unknown as { path?: string }).path ?? first.name;
    setPending({
      name: first.name.replace(/\.[^/.]+$/, ""),
      path: filePath,
      kind: guessKind(first.name),
    });
    setErrorMsg(null);
  };

  // ── Render ─────────────────────────────────────────────────────────────────

  const entries: AssetLibEntry[] = query.data ?? [];

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
        <div>
          <h1 className="text-[var(--text-ui-xl)] font-bold">
            Ressursbibliotek
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Logoer, maler, sangark, skrifttyper og gjenbruksblokker
          </p>
        </div>
        {query.isPending && (
          <Loader2
            size={16}
            className="animate-spin text-[var(--color-fg-muted)]"
          />
        )}
      </header>

      {/* Filter bar */}
      <div className="flex gap-1.5 overflow-x-auto border-b border-[var(--color-border)] px-6 py-2.5">
        {FILTER_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => setKindFilter(opt.value)}
            className={cn(
              "shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors",
              kindFilter === opt.value
                ? "bg-[var(--color-accent)] text-[var(--color-accent-fg)]"
                : "border border-[var(--color-border)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Main scrollable area */}
      <div className="flex-1 overflow-y-auto p-6">
        {/* Error banner */}
        {errorMsg && (
          <div className="mb-4 rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-4 py-2.5 text-sm text-[var(--color-danger)]">
            {errorMsg}
            <button
              type="button"
              className="ml-3 text-xs underline"
              onClick={() => setErrorMsg(null)}
            >
              Lukk
            </button>
          </div>
        )}

        {/* Inline add form when a file is pending */}
        {pending && (
          <div className="mb-5">
            <AddForm
              fileName={pending.name}
              filePath={pending.path}
              defaultKind={pending.kind}
              isPending={addMutation.isPending}
              onCancel={() => {
                setPending(null);
                setErrorMsg(null);
              }}
              onSubmit={({ name, kind, tags }) => {
                addMutation.mutate({
                  name,
                  kind,
                  filePath: pending.path,
                  tags,
                });
              }}
            />
          </div>
        )}

        {/* Drop zone */}
        {!pending && (
          <div className="mb-5">
            <DropZone
              isDragOver={dragOver}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
              onDrop={handleDrop}
              onFiles={processFiles}
            />
          </div>
        )}

        {/* Grid */}
        {query.isError ? (
          <div className="flex flex-col items-center gap-3 py-16 text-center">
            <FolderOpen
              size={40}
              className="text-[var(--color-danger)] opacity-60"
            />
            <p className="text-sm text-[var(--color-danger)]">
              Kunne ikke laste biblioteket:{" "}
              {query.error instanceof IPCError
                ? `${query.error.code} — ${query.error.message}`
                : String(query.error)}
            </p>
          </div>
        ) : entries.length === 0 && !query.isPending ? (
          <div className="flex flex-col items-center gap-3 py-16 text-center">
            <FolderOpen
              size={40}
              className="text-[var(--color-fg-muted)] opacity-40"
            />
            <p className="text-sm text-[var(--color-fg-muted)]">
              {kindFilter === "all"
                ? "Biblioteket er tomt. Dra filer hit for å komme i gang."
                : `Ingen ${KIND_LABELS[kindFilter as AssetKind].toLowerCase()}-er i biblioteket ennå.`}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
            {entries.map((entry) => (
              <AssetCard
                key={entry.id}
                entry={entry}
                isDeleting={
                  deleteMutation.isPending &&
                  deleteMutation.variables === entry.id
                }
                onDelete={(id) => deleteMutation.mutate(id)}
                onOpen={(id) => openMutation.mutate(id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
