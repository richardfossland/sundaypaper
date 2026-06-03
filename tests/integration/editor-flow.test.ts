// Integration smoke — the document EDITOR flow at the IPC layer.
// Mocks Tauri's `invoke` and drives the sequence the Editor runs over an
// existing document: list blocks → create → update → delete → render → compile.
// Mirrors builder-pipeline.test.ts (mock invoke, assert command name + args).
import { describe, it, expect, vi, beforeEach } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { ipc, IPCError } from "@/lib/ipc";
import type { Block } from "@/lib/bindings";

const mkBlock = (over: Partial<Block>): Block => ({
  id: "b-1",
  document_id: "doc-1",
  parent_id: null,
  kind: "text",
  position: 0n,
  data: "{}",
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

describe("editor IPC flow", () => {
  beforeEach(() => invokeMock.mockReset());

  it("runs list → create → update → delete and threads each command's args", async () => {
    const existing = mkBlock({ id: "b-existing", kind: "heading" });
    const created = mkBlock({ id: "b-new" });
    const updated = mkBlock({
      id: "b-new",
      kind: "song",
      data: '{"title":"Salme"}',
    });

    invokeMock
      .mockResolvedValueOnce([existing]) // block_list
      .mockResolvedValueOnce(created) // block_create
      .mockResolvedValueOnce(updated) // block_update
      .mockResolvedValueOnce(undefined); // block_delete

    const list = await ipc.block.list("doc-1");
    expect(list).toHaveLength(1);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "block_list", {
      documentId: "doc-1",
    });

    const newBlock = await ipc.block.create("doc-1", null, "text", "{}");
    expect(newBlock.id).toBe("b-new");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "block_create", {
      documentId: "doc-1",
      parentId: null,
      kind: "text",
      data: "{}",
    });

    const edited = await ipc.block.update("b-new", "song", '{"title":"Salme"}');
    expect(edited.kind).toBe("song");
    expect(invokeMock).toHaveBeenNthCalledWith(3, "block_update", {
      id: "b-new",
      kind: "song",
      data: '{"title":"Salme"}',
    });

    await ipc.block.delete("b-new");
    expect(invokeMock).toHaveBeenNthCalledWith(4, "block_delete", {
      id: "b-new",
    });
  });

  it("render→compile chain matches the builder (bulletin_render → typst_compile)", async () => {
    invokeMock
      .mockResolvedValueOnce("#set page()\nHei") // bulletin_render
      .mockResolvedValueOnce("JVBERi0xLjQK"); // typst_compile → base64 PDF

    const source = await ipc.bulletin.render("doc-1");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "bulletin_render", {
      documentId: "doc-1",
      layoutMeta: undefined,
    });

    const pdf = await ipc.bulletin.typstCompile(source);
    expect(pdf).toBe("JVBERi0xLjQK");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "typst_compile", {
      source: "#set page()\nHei",
    });
  });

  it("propagates a backend rejection (missing doc) as an IPCError", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "not_found",
      message: "document missing",
    });
    let caught: unknown;
    try {
      await ipc.block.list("nope");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(IPCError);
    expect((caught as IPCError).code).toBe("not_found");
  });
});
