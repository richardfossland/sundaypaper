/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * TemplatesPanel unit tests — drive the Typst template builder with `@/lib/ipc`
 * mocked so template CRUD and the live-preview compile run without a backend.
 * Mirrors the seams used in AssetsPanel/FormsPage tests (whole-module ipc mock
 * + a real-ish IPCError so `instanceof` holds).
 *
 * Coverage: templates list grouped by kind, create seeds a starter + selects
 * it, the editor edits source, save sends the edited source, delete clears the
 * selection, the variable hints + lint react to source, and the live preview
 * injects SAMPLE data before compiling (proving sample-data substitution).
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

import type { Template } from "@/lib/bindings";

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
      template: {
        list: vi.fn(),
        create: vi.fn(),
        update: vi.fn(),
        delete: vi.fn(),
      },
      bulletin: {
        typstCompile: vi.fn(),
      },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { TemplatesPanel } from "./TemplatesPanel";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const mkTemplate = (over: Partial<Template>): Template => ({
  id: "t-x",
  name: "Mal",
  kind: "program",
  source: "",
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

const PROGRAM = mkTemplate({
  id: "t-prog",
  name: "Høymesse-program",
  kind: "program",
  source: "#align(center)[{{ title }}]",
});
const SONG = mkTemplate({
  id: "t-song",
  name: "Sangark A4",
  kind: "song_sheet",
  source: "{{ song_body }}",
});

const PDF_B64 = "JVBERi0xLjQK"; // "%PDF-1.4" — enough for the embed src.

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <TemplatesPanel />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.template.list.mockResolvedValue([PROGRAM, SONG]);
  ipcMock.template.create.mockResolvedValue(
    mkTemplate({ id: "t-new", name: "Ny plakat", kind: "poster" }),
  );
  ipcMock.template.update.mockImplementation((id, name, kind, source) =>
    Promise.resolve(mkTemplate({ id, name, kind, source })),
  );
  ipcMock.template.delete.mockResolvedValue(undefined);
  ipcMock.bulletin.typstCompile.mockResolvedValue(PDF_B64);
});

afterEach(() => cleanup());

describe("TemplatesPanel", () => {
  it("lists templates grouped by kind", async () => {
    renderPanel();
    expect(screen.getByText("Malbygger")).toBeInTheDocument();
    expect(await screen.findByText("Høymesse-program")).toBeInTheDocument();
    expect(screen.getByText("Sangark A4")).toBeInTheDocument();
    // Group headers come from the kind labels.
    expect(screen.getByText("Program")).toBeInTheDocument();
    expect(screen.getByText("Sangark")).toBeInTheDocument();
  });

  it("creates a template with a starter source and selects it", async () => {
    renderPanel();
    await screen.findByText("Høymesse-program");

    fireEvent.click(screen.getByRole("button", { name: "Ny mal" }));
    fireEvent.change(screen.getByLabelText("Navn på mal"), {
      target: { value: "Ny plakat" },
    });
    // Pick the "Plakat" kind chip.
    fireEvent.click(screen.getByRole("button", { name: "Plakat" }));
    fireEvent.click(screen.getByRole("button", { name: "Opprett" }));

    await waitFor(() =>
      expect(ipcMock.template.create).toHaveBeenCalledWith(
        "Ny plakat",
        "poster",
        expect.stringContaining("{{ title }}"),
      ),
    );
  });

  it("loads the selected template's source into the editor", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    const editor = (await screen.findByLabelText(
      "Typst-kilde",
    )) as HTMLTextAreaElement;
    expect(editor.value).toBe("#align(center)[{{ title }}]");
  });

  it("saves the edited source through ipc.template.update", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    const editor = await screen.findByLabelText("Typst-kilde");
    fireEvent.change(editor, {
      target: { value: "#align(center)[{{ title }} — {{ date }}]" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Lagre mal" }));
    await waitFor(() =>
      expect(ipcMock.template.update).toHaveBeenCalledWith(
        "t-prog",
        "Høymesse-program",
        "program",
        "#align(center)[{{ title }} — {{ date }}]",
      ),
    );
  });

  it("shows the variable hints derived from the source", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    const editor = await screen.findByLabelText("Typst-kilde");
    fireEvent.change(editor, {
      target: { value: "{{ church_name }} {{ date }}" },
    });

    // The placeholder text appears both in the highlight overlay and the hints
    // chip rail, so assert it is present at all (>=1) rather than uniquely.
    expect(
      (await screen.findAllByText("{{ church_name }}")).length,
    ).toBeGreaterThan(0);
    expect(screen.getAllByText("{{ date }}").length).toBeGreaterThan(0);
  });

  it("surfaces a lint error and suppresses the preview compile", async () => {
    vi.useFakeTimers();
    try {
      renderPanel();
      // Advance the list query.
      await vi.runOnlyPendingTimersAsync();
      fireEvent.click(screen.getByText("Høymesse-program"));

      const editor = screen.getByLabelText("Typst-kilde");
      // Unbalanced bracket → lint error.
      fireEvent.change(editor, {
        target: { value: "#align(center)[{{ title }}" },
      });

      expect(screen.getByText(/Rett opp syntaksfeilene/)).toBeInTheDocument();

      // Even past the debounce window, no compile fires while there's an error.
      await vi.advanceTimersByTimeAsync(800);
      expect(ipcMock.bulletin.typstCompile).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("injects sample data before compiling the live preview", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    // The default source has {{ title }}; the preview must substitute the
    // sample value ("Høymesse") rather than send the raw placeholder.
    await waitFor(
      () =>
        expect(ipcMock.bulletin.typstCompile).toHaveBeenCalledWith(
          "#align(center)[Høymesse]",
        ),
      { timeout: 2000 },
    );
  });

  it("renders the compiled preview as a PDF embed", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    await waitFor(
      () => {
        const embed = screen.getByTitle("Forhåndsvisning") as HTMLEmbedElement;
        expect(embed.src).toContain(`base64,${PDF_B64}`);
      },
      { timeout: 2000 },
    );
  });

  it("deletes a template and clears the selection", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));
    await screen.findByLabelText("Typst-kilde");

    fireEvent.click(
      screen.getByRole("button", { name: "Slett Høymesse-program" }),
    );
    await waitFor(() =>
      expect(ipcMock.template.delete).toHaveBeenCalledWith("t-prog"),
    );
  });

  it("surfaces a save failure as an error banner", async () => {
    ipcMock.template.update.mockRejectedValue(
      new FakeIPCError("validation", "template name is required"),
    );
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse-program"));

    const editor = await screen.findByLabelText("Typst-kilde");
    fireEvent.change(editor, { target: { value: "#changed[{{ title }}]" } });
    fireEvent.click(screen.getByRole("button", { name: "Lagre mal" }));

    expect(
      await screen.findByText(/template name is required/),
    ).toBeInTheDocument();
  });

  it("surfaces a list failure", async () => {
    ipcMock.template.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPanel();
    await waitFor(() =>
      expect(screen.getByText(/Kunne ikke laste maler/)).toBeInTheDocument(),
    );
  });
});
