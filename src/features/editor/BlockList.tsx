/**
 * BlockList — the center column of the editor. Renders a document's flat block
 * list (from `ipc.block.list`) as a hierarchy: blocks are grouped by
 * `parent_id` and ordered by `position`, then walked depth-first so children
 * appear indented under their parent. Each row is a BlockCard.
 *
 * The list is intentionally read-only here; all mutations (create / update /
 * delete / move) bubble up to EditorPage, which owns the query + ipc calls so
 * invalidation lives in one place.
 */

import { Plus } from "lucide-react";

import type { Block } from "@/lib/bindings";
import { BlockCard } from "./BlockCard";
import { flattenHierarchy } from "./flatten-hierarchy";

interface BlockListProps {
  blocks: Block[];
  busy: boolean;
  onAdd: () => void;
  onUpdate: (id: string, kind: string, data: string) => void;
  onDelete: (id: string) => void;
}

export function BlockList({
  blocks,
  busy,
  onAdd,
  onUpdate,
  onDelete,
}: BlockListProps) {
  const flat = flattenHierarchy(blocks);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center justify-between pb-3">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
          Blokker ({blocks.length})
        </h2>
        <button
          type="button"
          disabled={busy}
          onClick={onAdd}
          className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
        >
          <Plus size={13} />
          Legg til blokk
        </button>
      </div>

      {flat.length === 0 ? (
        <p className="text-sm text-[var(--color-fg-muted)]">
          Dokumentet har ingen blokker ennå — legg til den første.
        </p>
      ) : (
        <ul className="flex-1 space-y-2 overflow-y-auto pb-4">
          {flat.map(({ block, depth }) => (
            <li key={block.id}>
              <BlockCard
                block={block}
                depth={depth}
                busy={busy}
                onUpdate={onUpdate}
                onDelete={onDelete}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
