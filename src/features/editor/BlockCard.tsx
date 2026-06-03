/**
 * BlockCard — the per-block editor row used inside BlockList.
 *
 * A document block is `{ kind, data }` where `data` is a JSON string the
 * layout engine understands. We expose both: a `kind` selector (the kinds the
 * renderer already handles) and a raw JSON `data` textarea. JSON is validated
 * locally before we hand it to `ipc.block.update` so the user sees a clear
 * inline error instead of a backend rejection.
 *
 * The card is uncontrolled-ish: it seeds local draft state from the block, and
 * a "Lagre" button commits via the parent's `onUpdate`. Delete is immediate
 * (it doesn't depend on the draft).
 */

import { useEffect, useState } from "react";
import { AlertCircle, Save, Trash2 } from "lucide-react";

import type { Block } from "@/lib/bindings";
import { cn } from "@/lib/cn";

/** Block kinds the layout engine renders today (see services/bulletin.rs). */
export const BLOCK_KINDS = [
  "heading",
  "text",
  "song",
  "scripture",
  "liturgy",
  "announcement",
] as const;

/** Validate a string is parseable JSON; returns an error message or null. */
export function jsonError(raw: string): string | null {
  if (raw.trim() === "") return null; // empty is allowed (defaults to "{}")
  try {
    JSON.parse(raw);
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : "Ugyldig JSON";
  }
}

interface BlockCardProps {
  block: Block;
  /** Depth in the parent_id hierarchy; indents the card. 0 = top level. */
  depth: number;
  busy: boolean;
  onUpdate: (id: string, kind: string, data: string) => void;
  onDelete: (id: string) => void;
}

export function BlockCard({
  block,
  depth,
  busy,
  onUpdate,
  onDelete,
}: BlockCardProps) {
  const [kind, setKind] = useState(block.kind);
  const [data, setData] = useState(block.data);

  // Re-seed the draft if the underlying block changes identity/content
  // (e.g. after a successful save refetch).
  useEffect(() => {
    setKind(block.kind);
    setData(block.data);
  }, [block.id, block.kind, block.data]);

  const invalid = jsonError(data);
  const dirty = kind !== block.kind || data !== block.data;

  return (
    <div
      className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3"
      style={{ marginLeft: depth * 16 }}
      data-block-id={block.id}
      data-depth={depth}
    >
      <div className="flex items-center gap-2">
        <select
          aria-label="Blokktype"
          value={kind}
          onChange={(e) => setKind(e.target.value)}
          disabled={busy}
          className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2 py-1 text-xs font-medium"
        >
          {/* Keep the current kind selectable even if it's a custom one. */}
          {!BLOCK_KINDS.includes(kind as (typeof BLOCK_KINDS)[number]) && (
            <option value={kind}>{kind}</option>
          )}
          {BLOCK_KINDS.map((k) => (
            <option key={k} value={k}>
              {k}
            </option>
          ))}
        </select>

        <span className="flex-1" />

        <button
          type="button"
          aria-label="Slett blokk"
          disabled={busy}
          onClick={() => onDelete(block.id)}
          className="rounded-md border border-[var(--color-border)] p-1 text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-danger)] disabled:opacity-40"
        >
          <Trash2 size={14} />
        </button>
      </div>

      <textarea
        aria-label="Blokkdata (JSON)"
        value={data}
        onChange={(e) => setData(e.target.value)}
        disabled={busy}
        spellCheck={false}
        rows={3}
        className={cn(
          "mt-2 w-full resize-y rounded-md border bg-[var(--color-bg-elevated)] px-2.5 py-1.5 font-mono text-xs",
          invalid
            ? "border-[var(--color-danger)]"
            : "border-[var(--color-border)]",
        )}
      />

      {invalid && (
        <p
          role="alert"
          className="mt-1 flex items-start gap-1.5 text-xs text-[var(--color-danger)]"
        >
          <AlertCircle size={12} className="mt-0.5 shrink-0" />
          {invalid}
        </p>
      )}

      <div className="mt-2 flex justify-end">
        <button
          type="button"
          disabled={busy || !dirty || !!invalid}
          onClick={() =>
            onUpdate(block.id, kind, data.trim() === "" ? "{}" : data)
          }
          className="flex items-center gap-1.5 rounded-md border border-[var(--color-accent)] px-2.5 py-1 text-xs font-semibold text-[var(--color-accent)] transition-colors hover:bg-[color-mix(in_oklch,var(--color-accent)_10%,transparent)] disabled:opacity-40"
        >
          <Save size={12} />
          Lagre
        </button>
      </div>
    </div>
  );
}
