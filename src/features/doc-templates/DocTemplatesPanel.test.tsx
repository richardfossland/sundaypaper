/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * DocTemplatesPanel unit tests — drive the document-template UI with `@/lib/ipc`
 * mocked so list/create/update/delete/seedBuiltins run without a backend.
 * Mirrors the seams in SongsPanel.test.tsx (whole-module ipc mock + a real-ish
 * IPCError so `instanceof` holds).
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

import type { DocTemplate } from "@/lib/bindings";

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
      docTemplate: {
        list: vi.fn(),
        create: vi.fn(),
        update: vi.fn(),
        delete: vi.fn(),
        seedBuiltins: vi.fn(),
      },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
  errMessage: (err: unknown, fallback: string) =>
    err instanceof Error ? err.message : fallback,
}));

import { DocTemplatesPanel } from "./DocTemplatesPanel";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const mkTemplate = (over: Partial<DocTemplate>): DocTemplate => ({
  id: "t-x",
  name: "Mal",
  kind: "Bulletin",
  typst_source: "{{title}}",
  preview_png: null,
  variables: [],
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

const HOYMESSE = mkTemplate({
  id: "t-hoymesse",
  name: "Høymesse",
  kind: "Bulletin",
  variables: [
    {
      id: "v-1",
      template_id: "t-hoymesse",
      name: "title",
      label: "Tittel",
      kind: "Text",
      default_value: null,
      required: true,
      position: 0n,
      created_at: 0n,
    },
  ],
});

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <DocTemplatesPanel />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.docTemplate.list.mockResolvedValue([HOYMESSE]);
  ipcMock.docTemplate.create.mockResolvedValue(
    mkTemplate({ id: "t-new", name: "Ny" }),
  );
  ipcMock.docTemplate.update.mockResolvedValue(HOYMESSE);
  ipcMock.docTemplate.delete.mockResolvedValue(undefined);
  ipcMock.docTemplate.seedBuiltins.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("DocTemplatesPanel", () => {
  it("lists templates with their kind label and variable count", async () => {
    renderPanel();
    expect(screen.getByText("Dokumentmaler")).toBeInTheDocument();
    expect(await screen.findByText("Høymesse")).toBeInTheDocument();
    // Norwegian kind label + variable count subline
    expect(screen.getByText(/Program · 1 variabler/)).toBeInTheDocument();
  });

  it("seeds the built-ins once when the list is empty, then refetches", async () => {
    // First load empty, after seeding the refetch returns a template.
    ipcMock.docTemplate.list
      .mockResolvedValueOnce([])
      .mockResolvedValue([HOYMESSE]);
    renderPanel();
    await waitFor(() =>
      expect(ipcMock.docTemplate.seedBuiltins).toHaveBeenCalledTimes(1),
    );
    expect(await screen.findByText("Høymesse")).toBeInTheDocument();
  });

  it("creates a template with the full variable list", async () => {
    renderPanel();
    await screen.findByText("Høymesse");

    fireEvent.click(screen.getByRole("button", { name: "Ny mal" }));
    fireEvent.change(screen.getByPlaceholderText("Høymesse-program"), {
      target: { value: "  Min mal  " },
    });
    // Add a variable row.
    fireEvent.click(screen.getByRole("button", { name: "Legg til" }));
    fireEvent.change(screen.getByLabelText("Variabelnavn 1"), {
      target: { value: " subtitle " },
    });

    fireEvent.click(screen.getByRole("button", { name: "Lagre" }));

    await waitFor(() =>
      expect(ipcMock.docTemplate.create).toHaveBeenCalledWith(
        "Min mal",
        "Bulletin",
        expect.any(String),
        [
          expect.objectContaining({
            name: "subtitle",
            label: "subtitle",
            kind: "Text",
            default_value: null,
            required: false,
          }),
        ],
      ),
    );
  });

  it("disables save when the name is blank", async () => {
    renderPanel();
    await screen.findByText("Høymesse");
    fireEvent.click(screen.getByRole("button", { name: "Ny mal" }));
    expect(screen.getByRole("button", { name: "Lagre" })).toBeDisabled();
  });

  it("edits an existing template via ipc.docTemplate.update (name/kind/source only)", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse"));

    const name = screen.getByDisplayValue("Høymesse");
    fireEvent.change(name, { target: { value: "Høymesse (rev)" } });
    fireEvent.click(screen.getByRole("button", { name: "Lagre" }));

    await waitFor(() =>
      expect(ipcMock.docTemplate.update).toHaveBeenCalledWith(
        "t-hoymesse",
        "Høymesse (rev)",
        "Bulletin",
        expect.any(String),
      ),
    );
  });

  it("shows existing variables read-only when editing", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse"));
    // Variable name input is present but disabled for an existing template.
    expect(screen.getByLabelText("Variabelnavn 1")).toBeDisabled();
    expect(
      screen.getByText(/Variabler kan bare settes når malen opprettes/),
    ).toBeInTheDocument();
  });

  it("deletes only after the confirm step", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Høymesse"));

    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    expect(ipcMock.docTemplate.delete).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    await waitFor(() =>
      expect(ipcMock.docTemplate.delete).toHaveBeenCalledWith("t-hoymesse"),
    );
  });

  it("surfaces a list failure as an error state", async () => {
    ipcMock.docTemplate.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPanel();
    await waitFor(() =>
      expect(
        screen.getByText(/Kunne ikke laste dokumentmaler/),
      ).toBeInTheDocument(),
    );
  });
});
