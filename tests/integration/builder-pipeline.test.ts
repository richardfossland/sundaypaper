// Integration smoke — the FORWARD bulletin pipeline at the IPC layer.
// Mocks Tauri's `invoke` and drives the three-command sequence the
// Document Builder runs: bulletin_generate → bulletin_render → typst_compile.
// Mirrors the pattern in ipc.test.ts (mock invoke, assert command + args).
import { describe, it, expect, vi, beforeEach } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { ipc, IPCError } from "@/lib/ipc";
import type { Document, ServicePlan } from "@/lib/bindings";

const PLAN: ServicePlan = {
  title: "Høymesse",
  church: "Vår Frelsers menighet",
  date: "1. juni 2026",
  items: [
    {
      kind: "welcome",
      title: "Velkommen",
      body: null,
      leader: "Liturg",
      time: null,
      copyright: null,
      page_break: false,
      song: null,
      scripture: null,
      asset: null,
    },
  ],
};

const DOC: Document = {
  id: "doc-1",
  project_id: "proj-1",
  template_id: null,
  title: "Høymesse",
  kind: "program",
  page_size: "a4",
  position: 0n,
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
};

describe("bulletin FORWARD pipeline", () => {
  beforeEach(() => invokeMock.mockReset());

  it("runs generate → render → typst_compile and threads the values", async () => {
    invokeMock
      .mockResolvedValueOnce(DOC) // bulletin_generate
      .mockResolvedValueOnce("#set page()\nHei") // bulletin_render
      .mockResolvedValueOnce("JVBERi0xLjQK"); // typst_compile → base64 PDF

    const doc = await ipc.bulletin.generate("proj-1", PLAN, PLAN.title!);
    expect(doc.id).toBe("doc-1");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "bulletin_generate", {
      projectId: "proj-1",
      plan: PLAN,
      title: "Høymesse",
    });

    const source = await ipc.bulletin.render(doc.id);
    expect(source).toContain("#set page()");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "bulletin_render", {
      documentId: "doc-1",
      layoutMeta: undefined,
    });

    const pdf = await ipc.bulletin.typstCompile(source);
    expect(pdf).toBe("JVBERi0xLjQK");
    expect(invokeMock).toHaveBeenNthCalledWith(3, "typst_compile", {
      source: "#set page()\nHei",
    });
  });

  it("propagates a feature_disabled compile error as an IPCError", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "feature_disabled",
      message: "typst feature off",
    });
    let caught: unknown;
    try {
      await ipc.bulletin.typstCompile("anything");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(IPCError);
    expect((caught as IPCError).code).toBe("feature_disabled");
    expect((caught as IPCError).message).toBe("typst feature off");
  });
});
