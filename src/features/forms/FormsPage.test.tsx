/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * FormsPage unit tests — drive the FormBuilder (Phase 7.2) with `@/lib/ipc`
 * mocked so field CRUD, form creation and the render→compile chain run without
 * a backend, DB or Typst compiler. Mirrors EditorPage.test.tsx (whole-module
 * ipc mock + a real-ish IPCError so `instanceof` checks hold).
 *
 * Coverage: shell + privacy banner, create a new form document, the quick-add
 * palette adding each of the three field kinds with their JSON skeleton, the
 * render→compile preview, and a create-failure error surface.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
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
      document: { list: vi.fn(), create: vi.fn() },
      block: {
        list: vi.fn(),
        create: vi.fn(),
        update: vi.fn(),
        delete: vi.fn(),
      },
      bulletin: { render: vi.fn(), typstCompile: vi.fn() },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { FormsPage } from "./FormsPage";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PROJECT: Project = {
  id: "proj-1",
  name: "Sommer 2026",
  description: "",
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
};

const FORM_DOC: Document = {
  id: "form-1",
  project_id: "proj-1",
  template_id: null,
  title: "Påmeldingsskjema",
  kind: "form",
  page_size: "a4",
  position: 0n,
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
};

const mkBlock = (over: Partial<Block>): Block => ({
  id: "b-x",
  document_id: "form-1",
  parent_id: null,
  kind: "form_field",
  position: 0n,
  data: '{"label":"Navn","hint":null,"width":"full"}',
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

const PDF_B64 = "JVBERi0xLjQK"; // "%PDF-1.4\n"

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <FormsPage />
    </QueryClientProvider>,
  );
}

/** Pick the project, then open the existing form document. */
async function openForm() {
  await screen.findByRole("option", { name: "Sommer 2026" });
  fireEvent.change(screen.getByLabelText("Velg prosjekt"), {
    target: { value: "proj-1" },
  });
  const openBtn = await screen.findByRole("button", {
    name: /Åpne Påmeldingsskjema/,
  });
  fireEvent.click(openBtn);
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.project.list.mockResolvedValue([PROJECT]);
  ipcMock.document.list.mockResolvedValue([FORM_DOC]);
  ipcMock.document.create.mockResolvedValue(FORM_DOC);
  ipcMock.block.list.mockResolvedValue([mkBlock({ id: "b-1" })]);
  ipcMock.block.create.mockResolvedValue(mkBlock({ id: "b-new" }));
  ipcMock.block.update.mockResolvedValue(mkBlock({ id: "b-1" }));
  ipcMock.block.delete.mockResolvedValue(undefined);
  ipcMock.bulletin.render.mockResolvedValue("#set page()\n#bp-field([Navn])");
  ipcMock.bulletin.typstCompile.mockResolvedValue(PDF_B64);
});

afterEach(() => cleanup());

describe("FormsPage", () => {
  it("renders the form-builder shell and the privacy promise", async () => {
    renderPage();
    expect(screen.getByText("Skjemabygger")).toBeInTheDocument();
    expect(screen.getByText(/forlater aldri maskinen/)).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "Sommer 2026" }),
    ).toBeInTheDocument();
  });

  it("creates a new form document via ipc.document.create", async () => {
    renderPage();
    await screen.findByRole("option", { name: "Sommer 2026" });
    fireEvent.change(screen.getByLabelText("Velg prosjekt"), {
      target: { value: "proj-1" },
    });

    fireEvent.click(await screen.findByRole("button", { name: /Nytt skjema/ }));
    await waitFor(() =>
      expect(ipcMock.document.create).toHaveBeenCalledWith(
        "proj-1",
        "Nytt skjema",
        "form",
        "A4",
      ),
    );
  });

  it("loads a form's fields once a form is opened", async () => {
    renderPage();
    await openForm();
    await waitFor(() =>
      expect(ipcMock.block.list).toHaveBeenCalledWith("form-1"),
    );
    expect(await screen.findByText(/Blokker \(1\)/)).toBeInTheDocument();
  });

  it("quick-adds a text field with its JSON skeleton", async () => {
    renderPage();
    await openForm();
    await screen.findByText(/Blokker \(1\)/);

    fireEvent.click(screen.getByRole("button", { name: /Tekstfelt/ }));
    await waitFor(() =>
      expect(ipcMock.block.create).toHaveBeenCalledWith(
        "form-1",
        null,
        "form_field",
        JSON.stringify({ label: "Navn", hint: null, width: "full" }),
      ),
    );
  });

  it("quick-adds a checkbox field", async () => {
    renderPage();
    await openForm();
    await screen.findByText(/Blokker \(1\)/);

    fireEvent.click(screen.getByRole("button", { name: /Avkrysning/ }));
    await waitFor(() =>
      expect(ipcMock.block.create).toHaveBeenCalledWith(
        "form-1",
        null,
        "checkbox",
        JSON.stringify({ label: "Jeg samtykker" }),
      ),
    );
  });

  it("quick-adds a signature field", async () => {
    renderPage();
    await openForm();
    await screen.findByText(/Blokker \(1\)/);

    fireEvent.click(screen.getByRole("button", { name: /Signatur/ }));
    await waitFor(() =>
      expect(ipcMock.block.create).toHaveBeenCalledWith(
        "form-1",
        null,
        "signature",
        JSON.stringify({ label: "Signatur og dato", width: "half" }),
      ),
    );
  });

  it("runs the render→compile chain and surfaces the preview", async () => {
    renderPage();
    await openForm();
    await screen.findByText(/Blokker \(1\)/);

    fireEvent.click(screen.getByRole("button", { name: /Forhåndsvis PDF/ }));

    await waitFor(() =>
      expect(ipcMock.bulletin.render).toHaveBeenCalledWith("form-1"),
    );
    await waitFor(() =>
      expect(ipcMock.bulletin.typstCompile).toHaveBeenCalledWith(
        "#set page()\n#bp-field([Navn])",
      ),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: /Last ned PDF/ }),
      ).toBeInTheDocument(),
    );
  });

  it("surfaces a create-form failure as an error banner", async () => {
    ipcMock.document.create.mockRejectedValue(
      new FakeIPCError("validation", "title is required"),
    );
    // Open an existing form first so the center column (which hosts the
    // mutation error banner) is mounted.
    renderPage();
    await openForm();
    await screen.findByText(/Blokker \(1\)/);

    fireEvent.click(screen.getByRole("button", { name: /Nytt skjema/ }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/validation/),
    );
  });
});
