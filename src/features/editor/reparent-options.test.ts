/**
 * reparent-options tests — the pure target-set computation that decides which
 * containers a block may be nested into (Step 2: block nesting). No DOM, no IPC.
 */
import { describe, it, expect } from "vitest";

import type { Block } from "@/lib/bindings";
import { reparentTargets, subtreeIds } from "./reparent-options";

const mk = (over: Partial<Block>): Block => ({
  id: "x",
  document_id: "d",
  parent_id: null,
  kind: "text",
  position: 0n,
  data: "{}",
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

describe("subtreeIds", () => {
  it("collects a node plus all of its descendants", () => {
    const blocks = [
      mk({ id: "root", kind: "two_column" }),
      mk({ id: "a", parent_id: "root" }),
      mk({ id: "b", parent_id: "root", kind: "callout" }),
      mk({ id: "b1", parent_id: "b" }),
      mk({ id: "loose" }),
    ];
    expect(subtreeIds(blocks, "root")).toEqual(
      new Set(["root", "a", "b", "b1"]),
    );
    expect(subtreeIds(blocks, "loose")).toEqual(new Set(["loose"]));
  });

  it("terminates on a pre-existing malformed cycle", () => {
    // p ↔ q reference each other; the walk must not loop forever.
    const blocks = [
      mk({ id: "p", parent_id: "q" }),
      mk({ id: "q", parent_id: "p" }),
    ];
    expect(subtreeIds(blocks, "p")).toEqual(new Set(["p", "q"]));
  });
});

describe("reparentTargets", () => {
  it("offers every container except the block's own subtree", () => {
    const blocks = [
      mk({ id: "col", kind: "two_column" }),
      mk({ id: "box", kind: "callout" }),
      mk({ id: "leaf", kind: "text" }),
    ];
    // A loose leaf may go into either container.
    expect(
      reparentTargets(blocks, "leaf")
        .map((t) => t.id)
        .sort(),
    ).toEqual(["box", "col"]);
  });

  it("never offers the block's own subtree (no cycles, no self)", () => {
    const blocks = [
      mk({ id: "outer", kind: "two_column" }),
      mk({ id: "inner", kind: "callout", parent_id: "outer" }),
      mk({ id: "deep", kind: "text", parent_id: "inner" }),
      // A second, unrelated container `inner` could legally move into.
      mk({ id: "sibling", kind: "callout" }),
    ];
    // `outer` cannot move under itself or under its descendants `inner`/`deep`;
    // `sibling` is the only valid destination.
    expect(reparentTargets(blocks, "outer").map((t) => t.id)).toEqual([
      "sibling",
    ]);
    // `inner`'s current parent `outer` is excluded (no-op); only `sibling`
    // remains (its own descendant `deep` is not a container anyway).
    expect(reparentTargets(blocks, "inner").map((t) => t.id)).toEqual([
      "sibling",
    ]);
  });

  it("excludes the block's current parent (a no-op move)", () => {
    const blocks = [
      mk({ id: "home", kind: "two_column" }),
      mk({ id: "elsewhere", kind: "callout" }),
      mk({ id: "child", kind: "text", parent_id: "home" }),
    ];
    // `child` is already in `home`, so only `elsewhere` is offered.
    expect(reparentTargets(blocks, "child").map((t) => t.id)).toEqual([
      "elsewhere",
    ]);
  });

  it("ignores non-container blocks as destinations", () => {
    const blocks = [
      mk({ id: "leaf1", kind: "text" }),
      mk({ id: "leaf2", kind: "song" }),
    ];
    expect(reparentTargets(blocks, "leaf1")).toEqual([]);
  });

  it("returns nothing for an unknown block id", () => {
    expect(reparentTargets([mk({ id: "a", kind: "callout" })], "nope")).toEqual(
      [],
    );
  });
});
