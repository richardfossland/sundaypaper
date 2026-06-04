/**
 * Hierarchy flattening for the block editor.
 *
 * Lives in a sibling module so BlockList.tsx only exports its component (keeps
 * React Fast Refresh working) and so this pure, load-bearing tree walk can be
 * unit-tested directly.
 */

import type { Block } from "@/lib/bindings";

/** A block paired with its computed tree depth, in render order. */
export interface FlatBlock {
  block: Block;
  depth: number;
}

/**
 * Turn a flat block list into a depth-first, position-ordered sequence with a
 * computed `depth` per node. Top-level blocks have `parent_id === null`. Pure
 * and exported so the tests can assert hierarchy ordering directly.
 */
export function flattenHierarchy(blocks: Block[]): FlatBlock[] {
  // Group children by parent id ("" stands in for the null/top-level bucket).
  const byParent = new Map<string, Block[]>();
  for (const b of blocks) {
    const key = b.parent_id ?? "";
    const bucket = byParent.get(key);
    if (bucket) bucket.push(b);
    else byParent.set(key, [b]);
  }
  for (const bucket of byParent.values()) {
    bucket.sort((a, b) => Number(a.position - b.position));
  }

  const out: FlatBlock[] = [];
  const walk = (parentKey: string, depth: number) => {
    for (const b of byParent.get(parentKey) ?? []) {
      out.push({ block: b, depth });
      walk(b.id, depth + 1); // descend into this block's children
    }
  };
  walk("", 0);
  return out;
}
