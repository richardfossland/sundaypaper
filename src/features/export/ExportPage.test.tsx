/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * ExportPage unit tests — drive the batch export page (Phase 6) with `@/lib/ipc`
 * mocked so the multi-select, option assembly, preview chain and the
 * `exporter.batch` mutation run without a backend, DB or Typst compiler. Mirrors
 * FormsPage.test.tsx (whole-module ipc mock + a real-ish IPCError so `instanceof`
 * checks hold).
 *
 * Coverage: shell, project → multi-select state, the export button gating on a
 * selection + a target folder, the batch command's argument shape (incl. the
 * large-print option), the success summary, the render→compile preview, and a
 * batch-failure error surface.
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

import type { BatchExportResult, Document, Project } from "@/lib/bindings";

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
      bulletin: { render: vi.fn(), typstCompile: vi.fn() },
      exporter: { batch: vi.fn() },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { ExportPage } from "./ExportPage";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PROJECT: Project = {
  id: "proj-1",
  name: "Sommer 2026",
  description: "",
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
};

const mkDoc = (over: Partial<Document>): Document => ({
  id: "doc-x",
  project_id: "proj-1",
  template_id: null,
  title: "Program",
  kind: "program",
  page_size: "a4",
  position: 0n,
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

const DOC_A = mkDoc({ id: "doc-a", title: "Søndag" });
const DOC_B = mkDoc({ id: "doc-b", title: "Kveldsbønn" });

const RESULT: BatchExportResult = {
  directory: "/tmp/out",
  files: [
    {
      documentId: "doc-a",
      path: "/tmp/out/Søndag.pdf",
      fileName: "Søndag.pdf",
    },
  ],
};

const PDF_B64 = "JVBERi0xLjQK"; // "%PDF-1.4\n"

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <ExportPage />
    </QueryClientProvider>,
  );
}

/** Pick the project so the document checklist loads. */
async function pickProject() {
  await screen.findByRole("option", { name: "Sommer 2026" });
  fireEvent.change(screen.getByLabelText("Velg prosjekt"), {
    target: { value: "proj-1" },
  });
  await screen.findByLabelText("Velg Søndag");
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.project.list.mockResolvedValue([PROJECT]);
  ipcMock.document.list.mockResolvedValue([DOC_A, DOC_B]);
  ipcMock.bulletin.render.mockResolvedValue("#set page()\nHello");
  ipcMock.bulletin.typstCompile.mockResolvedValue(PDF_B64);
  ipcMock.exporter.batch.mockResolvedValue(RESULT);
});

afterEach(() => cleanup());

describe("ExportPage", () => {
  it("renders the export shell", async () => {
    renderPage();
    expect(screen.getByText("Samleeksport")).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "Sommer 2026" }),
    ).toBeInTheDocument();
  });

  it("lists a project's documents as a multi-select once a project is picked", async () => {
    renderPage();
    await pickProject();
    expect(screen.getByLabelText("Velg Søndag")).toBeInTheDocument();
    expect(screen.getByLabelText("Velg Kveldsbønn")).toBeInTheDocument();
  });

  it("tracks multi-select state and reflects the count", async () => {
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.click(screen.getByLabelText("Velg Kveldsbønn"));
    expect(await screen.findByText(/\(2 valgt\)/)).toBeInTheDocument();

    // Toggling one off drops the count back.
    fireEvent.click(screen.getByLabelText("Velg Kveldsbønn"));
    expect(await screen.findByText(/\(1 valgt\)/)).toBeInTheDocument();
  });

  it("keeps the export button disabled until a doc and a folder are chosen", async () => {
    renderPage();
    await pickProject();

    const btn = screen.getByRole("button", { name: /Eksporter/ });
    expect(btn).toBeDisabled();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    expect(btn).toBeDisabled(); // still no folder

    fireEvent.change(screen.getByLabelText("Målmappe"), {
      target: { value: "/tmp/out" },
    });
    expect(btn).toBeEnabled();
  });

  it("calls exporter.batch with the selected ids, options and folder", async () => {
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.change(screen.getByLabelText("Målmappe"), {
      target: { value: "/tmp/out" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Eksporter/ }));

    await waitFor(() =>
      expect(ipcMock.exporter.batch).toHaveBeenCalledWith(
        ["doc-a"],
        { paper: null, largePrintPercent: null, lang: null },
        "/tmp/out",
      ),
    );
  });

  it("passes the large-print percent when the option is on", async () => {
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.change(screen.getByLabelText("Målmappe"), {
      target: { value: "/tmp/out" },
    });
    fireEvent.click(screen.getByLabelText("Storskrift"));
    // The slider appears once large-print is enabled; nudge it.
    fireEvent.change(screen.getByLabelText("Skalering i prosent"), {
      target: { value: "200" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Eksporter/ }));

    await waitFor(() =>
      expect(ipcMock.exporter.batch).toHaveBeenCalledWith(
        ["doc-a"],
        { paper: null, largePrintPercent: 200, lang: null },
        "/tmp/out",
      ),
    );
  });

  it("shows the success summary after a batch export", async () => {
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.change(screen.getByLabelText("Målmappe"), {
      target: { value: "/tmp/out" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Eksporter/ }));

    expect(
      await screen.findByText(/1 fil\(er\) eksportert/),
    ).toBeInTheDocument();
    expect(screen.getByText("/tmp/out")).toBeInTheDocument();
    expect(screen.getByText("Søndag.pdf")).toBeInTheDocument();
  });

  it("runs the render→compile chain for the first selected document preview", async () => {
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.click(
      await screen.findByRole("button", { name: /Forhåndsvis første/ }),
    );

    await waitFor(() =>
      expect(ipcMock.bulletin.render).toHaveBeenCalledWith("doc-a"),
    );
    await waitFor(() =>
      expect(ipcMock.bulletin.typstCompile).toHaveBeenCalledWith(
        "#set page()\nHello",
      ),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: /Last ned PDF/ }),
      ).toBeInTheDocument(),
    );
  });

  it("surfaces a batch-export failure as an error banner", async () => {
    ipcMock.exporter.batch.mockRejectedValue(
      new FakeIPCError("feature_disabled", "typst is not enabled"),
    );
    renderPage();
    await pickProject();

    fireEvent.click(screen.getByLabelText("Velg Søndag"));
    fireEvent.change(screen.getByLabelText("Målmappe"), {
      target: { value: "/tmp/out" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Eksporter/ }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/feature_disabled/),
    );
  });
});
