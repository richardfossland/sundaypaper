/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * BuilderPage unit tests — drive the FORWARD pipeline with `ipc.bulletin.*`
 * mocked so the three-command sequence (generate → render → typstCompile) runs
 * without a backend, DB, or Typst compiler.
 *
 * We mock the whole `@/lib/ipc` module (its `ipc` object + `IPCError`) so the
 * component's mutations resolve / reject under our control, then assert the UI.
 * Interactions use `fireEvent` (no `user-event` dependency in this repo).
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

import type { Document, Project } from "@/lib/bindings";

// ── ipc mock ──────────────────────────────────────────────────────────────────
// IPCError stays a real class so `instanceof` checks in the component hold.
// Hoisted alongside the mock so the `vi.mock` factory (also hoisted) can use it.
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
      project: { list: vi.fn(), create: vi.fn() },
      bulletin: { generate: vi.fn(), render: vi.fn(), typstCompile: vi.fn() },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { BuilderPage } from "./BuilderPage";

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

// A tiny valid-looking base64 string (decodes via atob in PdfPreview).
const PDF_B64 = "JVBERi0xLjQK"; // "%PDF-1.4\n"

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <BuilderPage />
    </QueryClientProvider>,
  );
}

/** Pick the project in the select. */
function selectProject() {
  fireEvent.change(screen.getByLabelText("Velg prosjekt"), {
    target: { value: "proj-1" },
  });
}

/** Seed the sample plan (gives the plan items + a title). */
function loadSample() {
  fireEvent.click(screen.getByRole("button", { name: /Last inn eksempel/ }));
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.project.list.mockResolvedValue([PROJECT]);
  ipcMock.bulletin.generate.mockResolvedValue(DOC);
  ipcMock.bulletin.render.mockResolvedValue("#set page()\nHei");
  ipcMock.bulletin.typstCompile.mockResolvedValue(PDF_B64);
});

afterEach(() => cleanup());

describe("BuilderPage", () => {
  it("renders the builder shell and loads projects", async () => {
    renderPage();
    expect(screen.getByText("Dokumentbygger")).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "Sommer 2026" }),
    ).toBeInTheDocument();
  });

  it("runs the happy path: plan → document → Typst → PDF", async () => {
    renderPage();

    await screen.findByRole("option", { name: "Sommer 2026" });
    selectProject();
    loadSample();

    fireEvent.click(screen.getByRole("button", { name: /Generer dokument/ }));
    await waitFor(() =>
      expect(ipcMock.bulletin.generate).toHaveBeenCalledWith(
        "proj-1",
        expect.objectContaining({ title: "Høymesse" }),
        "Høymesse",
      ),
    );

    // The document card + compile button appear once generate resolves.
    const makePdf = await screen.findByRole("button", { name: /Lag PDF/ });
    fireEvent.click(makePdf);

    // render then typstCompile, in sequence.
    await waitFor(() =>
      expect(ipcMock.bulletin.render).toHaveBeenCalledWith("doc-1"),
    );
    await waitFor(() =>
      expect(ipcMock.bulletin.typstCompile).toHaveBeenCalledWith(
        "#set page()\nHei",
      ),
    );

    // Preview surfaces (the download link lives in PdfPreview).
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: /Last ned PDF/ }),
      ).toBeInTheDocument(),
    );
  });

  it("blocks generate until a project is selected", async () => {
    renderPage();
    await screen.findByRole("option", { name: "Sommer 2026" });

    loadSample(); // items present, but no project picked
    expect(
      screen.getByRole("button", { name: /Generer dokument/ }),
    ).toBeDisabled();
    expect(ipcMock.bulletin.generate).not.toHaveBeenCalled();
  });

  it("disables generate when the plan has no items", async () => {
    renderPage();
    await screen.findByRole("option", { name: "Sommer 2026" });
    selectProject();

    // The default plan has exactly one row — remove it to empty the plan.
    fireEvent.click(screen.getByRole("button", { name: /Fjern post 1/ }));
    expect(
      screen.getByRole("button", { name: /Generer dokument/ }),
    ).toBeDisabled();
  });

  it("surfaces a compile failure as an error banner", async () => {
    ipcMock.bulletin.typstCompile.mockRejectedValue(
      new FakeIPCError("feature_disabled", "typst feature off"),
    );
    renderPage();

    await screen.findByRole("option", { name: "Sommer 2026" });
    selectProject();
    loadSample();
    fireEvent.click(screen.getByRole("button", { name: /Generer dokument/ }));

    const makePdf = await screen.findByRole("button", { name: /Lag PDF/ });
    fireEvent.click(makePdf);

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/feature_disabled/),
    );
    // No preview when compile failed.
    expect(
      screen.queryByRole("link", { name: /Last ned PDF/ }),
    ).not.toBeInTheDocument();
  });

  it("matches the empty-state snapshot", async () => {
    const { container } = renderPage();
    await screen.findByRole("option", { name: "Sommer 2026" });
    expect(container).toMatchSnapshot();
  });
});
