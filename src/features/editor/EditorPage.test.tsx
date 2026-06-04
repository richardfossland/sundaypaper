/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * EditorPage unit tests — drive the document-editor lifecycle with `@/lib/ipc`
 * mocked so block CRUD + the render→compile chain run without a backend, DB, or
 * Typst compiler. Mirrors the seams used in BuilderPage.test.tsx (whole-module
 * ipc mock + a real-ish IPCError class so `instanceof` checks hold).
 *
 * Coverage: load project → document → blocks, hierarchy grouping by parent_id,
 * add / update / delete a block, the render→compile PDF chain, and the two
 * error surfaces (missing/failed load, invalid JSON in a block's data field).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
  cleanup,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import type { Block, Document, Project } from "@/lib/bindings";

// ── ipc mock ──────────────────────────────────────────────────────────────────
const { ipcMock, FakeIPCError } = vi.hoisted(() => {
  class FakeIPCError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
      this.name = "IPCError";
    }
  }
  return {
    FakeIPCError,
    ipcMock: {
      project: { list: vi.fn() },
      document: { list: vi.fn() },
      block: {
        list: vi.fn(),
        create: vi.fn(),
        update: vi.fn(),
        reparent: vi.fn(),
        delete: vi.fn(),
      },
      bulletin: { render: vi.fn(), typstCompile: vi.fn() },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
  errMessage: (err: unknown, fallback: string) => {
    if (err instanceof FakeIPCError) return `${err.code} — ${err.message}`;
    if (err instanceof Error) return err.message;
    return fallback;
  },
}));

import { EditorPage } from "./EditorPage";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PROJECT: Project = {
  id: "proj-1",
  name: "Sommer 2026",
  description: "",
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
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

const mkBlock = (over: Partial<Block>): Block => ({
  id: "b-x",
  document_id: "doc-1",
  parent_id: null,
  kind: "text",
  position: 0n,
  data: "{}",
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

// A parent heading with one child song → exercises parent_id grouping.
const PARENT = mkBlock({ id: "b-parent", kind: "heading", position: 0n });
const CHILD = mkBlock({
  id: "b-child",
  kind: "song",
  parent_id: "b-parent",
  position: 0n,
  data: '{"title":"Navn over alle navn"}',
});

const PDF_B64 = "JVBERi0xLjQK"; // "%PDF-1.4\n"

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <EditorPage />
    </QueryClientProvider>,
  );
}

/** Pick the project, then the document — the common preamble for most tests. */
async function openDocument() {
  await screen.findByRole("option", { name: "Sommer 2026" });
  fireEvent.change(screen.getByLabelText("Velg prosjekt"), {
    target: { value: "proj-1" },
  });
  const openBtn = await screen.findByRole("button", { name: /Åpne Høymesse/ });
  fireEvent.click(openBtn);
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.project.list.mockResolvedValue([PROJECT]);
  ipcMock.document.list.mockResolvedValue([DOC]);
  ipcMock.block.list.mockResolvedValue([PARENT, CHILD]);
  ipcMock.block.create.mockResolvedValue(mkBlock({ id: "b-new" }));
  ipcMock.block.update.mockResolvedValue(PARENT);
  ipcMock.block.reparent.mockResolvedValue(CHILD);
  ipcMock.block.delete.mockResolvedValue(undefined);
  ipcMock.bulletin.render.mockResolvedValue("#set page()\nHei");
  ipcMock.bulletin.typstCompile.mockResolvedValue(PDF_B64);
});

afterEach(() => cleanup());

describe("EditorPage", () => {
  it("renders the editor shell and loads projects", async () => {
    renderPage();
    expect(screen.getByText("Dokumenteditor")).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "Sommer 2026" }),
    ).toBeInTheDocument();
  });

  it("loads a document's blocks once project + document are picked", async () => {
    renderPage();
    await openDocument();

    await waitFor(() =>
      expect(ipcMock.block.list).toHaveBeenCalledWith("doc-1"),
    );
    expect(await screen.findByText(/Blokker \(2\)/)).toBeInTheDocument();
  });

  it("visualises hierarchy: child blocks are indented under their parent", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    // The parent renders at depth 0, the child at depth 1 (data-depth attr).
    const parentCard = document.querySelector('[data-block-id="b-parent"]');
    const childCard = document.querySelector('[data-block-id="b-child"]');
    expect(parentCard).toHaveAttribute("data-depth", "0");
    expect(childCard).toHaveAttribute("data-depth", "1");
  });

  it("adds a top-level block via ipc.block.create", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    fireEvent.click(screen.getByRole("button", { name: /Legg til blokk/ }));
    await waitFor(() =>
      expect(ipcMock.block.create).toHaveBeenCalledWith(
        "doc-1",
        null,
        "text",
        "{}",
      ),
    );
  });

  it("updates a block's kind + data via ipc.block.update", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    const childCard = document.querySelector(
      '[data-block-id="b-child"]',
    ) as HTMLElement;
    const area = within(childCard).getByLabelText("Blokkdata (JSON)");
    fireEvent.change(area, { target: { value: '{"title":"Ny tittel"}' } });
    fireEvent.click(within(childCard).getByRole("button", { name: /Lagre/ }));

    await waitFor(() =>
      expect(ipcMock.block.update).toHaveBeenCalledWith(
        "b-child",
        "song",
        '{"title":"Ny tittel"}',
      ),
    );
  });

  it("blocks save + shows an inline alert when data is invalid JSON", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    const childCard = document.querySelector(
      '[data-block-id="b-child"]',
    ) as HTMLElement;
    const area = within(childCard).getByLabelText("Blokkdata (JSON)");
    fireEvent.change(area, { target: { value: "{ not json" } });

    expect(within(childCard).getByRole("alert")).toBeInTheDocument();
    expect(
      within(childCard).getByRole("button", { name: /Lagre/ }),
    ).toBeDisabled();
    expect(ipcMock.block.update).not.toHaveBeenCalled();
  });

  it("deletes a block via ipc.block.delete", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    const parentCard = document.querySelector(
      '[data-block-id="b-parent"]',
    ) as HTMLElement;
    fireEvent.click(
      within(parentCard).getByRole("button", { name: /Slett blokk/ }),
    );
    await waitFor(() =>
      expect(ipcMock.block.delete).toHaveBeenCalledWith("b-parent"),
    );
  });

  it("nests a block into a container via ipc.block.reparent", async () => {
    // A two_column container plus a loose text block the user nests under it.
    const CONTAINER = mkBlock({
      id: "b-col",
      kind: "two_column",
      position: 0n,
    });
    const LOOSE = mkBlock({ id: "b-loose", kind: "text", position: 1n });
    ipcMock.block.list.mockResolvedValue([CONTAINER, LOOSE]);

    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    const looseCard = document.querySelector(
      '[data-block-id="b-loose"]',
    ) as HTMLElement;
    // The move menu offers the container as a destination.
    const moveSelect = within(looseCard).getByLabelText(
      "Flytt blokk inn i beholder",
    );
    fireEvent.change(moveSelect, { target: { value: "b-col" } });

    await waitFor(() =>
      expect(ipcMock.block.reparent).toHaveBeenCalledWith("b-loose", "b-col"),
    );
  });

  it("does not offer a container as a destination for itself", async () => {
    const CONTAINER = mkBlock({
      id: "b-col",
      kind: "callout",
      position: 0n,
    });
    ipcMock.block.list.mockResolvedValue([CONTAINER]);

    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(1\)/);

    const card = document.querySelector(
      '[data-block-id="b-col"]',
    ) as HTMLElement;
    // A sole top-level container has nowhere to move (no other container, and
    // it's already top-level) → no move menu at all.
    expect(
      within(card).queryByLabelText("Flytt blokk inn i beholder"),
    ).toBeNull();
  });

  it("runs the render→compile chain and surfaces the preview", async () => {
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    fireEvent.click(screen.getByRole("button", { name: /Forhåndsvis PDF/ }));

    await waitFor(() =>
      expect(ipcMock.bulletin.render).toHaveBeenCalledWith("doc-1"),
    );
    await waitFor(() =>
      expect(ipcMock.bulletin.typstCompile).toHaveBeenCalledWith(
        "#set page()\nHei",
      ),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: /Last ned PDF/ }),
      ).toBeInTheDocument(),
    );
  });

  it("surfaces a block-load failure as an error banner", async () => {
    ipcMock.block.list.mockRejectedValue(
      new FakeIPCError("not_found", "document missing"),
    );
    renderPage();
    await openDocument();

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/not_found/),
    );
  });

  it("surfaces a compile failure as an error banner and shows no preview", async () => {
    ipcMock.bulletin.typstCompile.mockRejectedValue(
      new FakeIPCError("feature_disabled", "typst feature off"),
    );
    renderPage();
    await openDocument();
    await screen.findByText(/Blokker \(2\)/);

    fireEvent.click(screen.getByRole("button", { name: /Forhåndsvis PDF/ }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/feature_disabled/),
    );
    expect(
      screen.queryByRole("link", { name: /Last ned PDF/ }),
    ).not.toBeInTheDocument();
  });
});
