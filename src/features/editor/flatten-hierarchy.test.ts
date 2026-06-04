/**
 * flattenHierarchy unit tests — the pure tree walk that the editor's whole
 * hierarchy rendering depends on. Covers position ordering, depth-first
 * nesting, the computed per-node depth, and orphan handling.
 */
import { describe, it, expect } from "vitest";

import type { Block } from "@/lib/bindings";
import { flattenHierarchy } from "./flatten-hierarchy";

/** Build a Block with sensible defaults; only the tree-relevant fields matter. */
function mkBlock(
  id: string,
  parent_id: string | null,
  position: number,
): Block {
  return {
    id,
    document_id: "doc",
    parent_id,
    kind: "text",
    position: BigInt(position),
    data: "{}",
    created_at: 0n,
    updated_at: 0n,
  };
}

/** Pull just `[id, depth]` pairs so assertions read clearly. */
function shape(blocks: Block[]): Array<[string, number]> {
  return flattenHierarchy(blocks).map((f) => [f.block.id, f.depth]);
}

describe("flattenHierarchy", () => {
  it("returns an empty list for no blocks", () => {
    expect(flattenHierarchy([])).toEqual([]);
  });

  it("orders top-level blocks by position regardless of input order", () => {
    const blocks = [
      mkBlock("c", null, 2),
      mkBlock("a", null, 0),
      mkBlock("b", null, 1),
    ];
    expect(shape(blocks)).toEqual([
      ["a", 0],
      ["b", 0],
      ["c", 0],
    ]);
  });

  it("nests children depth-first under their parent with incremented depth", () => {
    const blocks = [
      mkBlock("root", null, 0),
      mkBlock("child1", "root", 0),
      mkBlock("child2", "root", 1),
      mkBlock("grandchild", "child1", 0),
    ];
    // root → child1 → grandchild (deepest first) → child2
    expect(shape(blocks)).toEqual([
      ["root", 0],
      ["child1", 1],
      ["grandchild", 2],
      ["child2", 1],
    ]);
  });

  it("orders sibling children by position too", () => {
    const blocks = [
      mkBlock("root", null, 0),
      mkBlock("second", "root", 5),
      mkBlock("first", "root", 1),
    ];
    expect(shape(blocks)).toEqual([
      ["root", 0],
      ["first", 1],
      ["second", 1],
    ]);
  });

  it("drops orphans whose parent is missing from the list", () => {
    const blocks = [
      mkBlock("root", null, 0),
      mkBlock("orphan", "ghost-parent", 0),
    ];
    // The orphan's parent never appears, so the walk never reaches it.
    expect(shape(blocks)).toEqual([["root", 0]]);
  });

  it("sorts by numeric position, not lexicographic bigint string", () => {
    const blocks = [mkBlock("ten", null, 10), mkBlock("two", null, 2)];
    expect(shape(blocks)).toEqual([
      ["two", 0],
      ["ten", 0],
    ]);
  });
});
