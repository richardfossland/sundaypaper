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
import { AlertCircle, IndentIncrease, Save, Trash2 } from "lucide-react";

import type { Block } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import {
  BLOCK_KINDS,
  blockKindLabel,
  isContainerKind,
  jsonError,
} from "./block-kinds";
import type { ReparentTarget } from "./reparent-options";
import { TableEditor } from "./TableEditor";

interface BlockCardProps {
  block: Block;
  /** Depth in the parent_id hierarchy; indents the card. 0 = top level. */
  depth: number;
  busy: boolean;
  /** Container blocks this card may be moved into (already excludes its own
   *  subtree + current parent). Empty → only the "move out" option shows. */
  reparentTargets: ReparentTarget[];
  onUpdate: (id: string, kind: string, data: string) => void;
  onReparent: (id: string, newParentId: string | null) => void;
  onDelete: (id: string) => void;
}

export function BlockCard({
  block,
  depth,
  busy,
  reparentTargets,
  onUpdate,
  onReparent,
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
              {blockKindLabel(k)}
            </option>
          ))}
        </select>

        {isContainerKind(block.kind) && (
          <span className="rounded-md bg-[color-mix(in_oklch,var(--color-accent)_12%,transparent)] px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-[var(--color-accent)]">
            Beholder
          </span>
        )}

        <span className="flex-1" />

        {/* Move-into-container menu: nest this block under a container, or move
            it back out to the top level. Only shown when there's somewhere to
            go (a container exists, or this block is itself nested). */}
        {(reparentTargets.length > 0 || block.parent_id !== null) && (
          <label className="flex items-center gap-1 text-[var(--color-fg-muted)]">
            <IndentIncrease size={13} aria-hidden />
            <select
              aria-label="Flytt blokk inn i beholder"
              value=""
              disabled={busy}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "") return;
                onReparent(block.id, v === "__root__" ? null : v);
              }}
              className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-1.5 py-1 text-xs"
            >
              <option value="">Flytt inn i…</option>
              {block.parent_id !== null && (
                <option value="__root__">Toppnivå (flytt ut)</option>
              )}
              {reparentTargets.map((t) => (
                <option key={t.id} value={t.id}>
                  {blockKindLabel(t.kind)} ({t.id.slice(0, 6)})
                </option>
              ))}
            </select>
          </label>
        )}

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

      {/* Tables get a structured grid editor; everything else keeps the raw
          JSON textarea. Both feed the same `data` draft + save flow. */}
      {kind === "table" ? (
        <TableEditor data={data} busy={busy} onChange={setData} />
      ) : (
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
      )}

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
