/**
 * Reparent target computation for the block editor (Step 2: block nesting).
 *
 * Lives in its own module so it stays pure and unit-testable: given the flat
 * block list and the block the user wants to move, it returns the container
 * blocks the move is *allowed* to land in. The backend re-validates (it owns the
 * source of truth and rejects self-parenting + cycles), but computing the valid
 * set up front means the UI never offers an option the backend would reject.
 */

import type { Block } from "@/lib/bindings";
import { isContainerKind } from "./block-kinds";

/** A move destination the editor can offer: a container block id + its label. */
export interface ReparentTarget {
  id: string;
  kind: string;
}

/**
 * Collect the ids in `block`'s subtree (itself + every descendant), so the
 * editor never offers to move a block under one of its own descendants (a
 * cycle) or under itself.
 */
export function subtreeIds(blocks: Block[], rootId: string): Set<string> {
  const childrenOf = new Map<string | null, Block[]>();
  for (const b of blocks) {
    const key = b.parent_id ?? null;
    const bucket = childrenOf.get(key);
    if (bucket) bucket.push(b);
    else childrenOf.set(key, [b]);
  }
  const ids = new Set<string>();
  const walk = (id: string) => {
    if (ids.has(id)) return; // guards against a pre-existing malformed cycle
    ids.add(id);
    for (const child of childrenOf.get(id) ?? []) walk(child.id);
  };
  walk(rootId);
  return ids;
}

/**
 * The container blocks `blockId` may be moved into: every container in the
 * document EXCEPT those in the block's own subtree (self + descendants) and
 * except the block's current parent (moving there would be a no-op).
 */
export function reparentTargets(
  blocks: Block[],
  blockId: string,
): ReparentTarget[] {
  const self = blocks.find((b) => b.id === blockId);
  if (!self) return [];
  const forbidden = subtreeIds(blocks, blockId);
  return blocks
    .filter(
      (b) =>
        isContainerKind(b.kind) &&
        !forbidden.has(b.id) &&
        b.id !== (self.parent_id ?? null),
    )
    .map((b) => ({ id: b.id, kind: b.kind }));
}
