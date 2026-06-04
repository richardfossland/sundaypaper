/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * SongsPanel unit tests — drive the song catalog UI with `@/lib/ipc` mocked so
 * list/create/update/delete run without a backend or DB. Mirrors the seams in
 * AssetsPanel.test.tsx (whole-module ipc mock + a real-ish IPCError so
 * `instanceof` holds).
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

import type { Song } from "@/lib/bindings";

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
      song: {
        list: vi.fn(),
        create: vi.fn(),
        update: vi.fn(),
        delete: vi.fn(),
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

import { SongsPanel } from "./SongsPanel";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const mkSong = (over: Partial<Song>): Song => ({
  id: "s-x",
  title: "Sang",
  author: null,
  body: "",
  language: null,
  tono_work_id: null,
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

const AMAZING = mkSong({
  id: "s-amazing",
  title: "Amazing Grace",
  author: "John Newton",
  body: "Amazing grace, how sweet the sound",
  language: "en",
});
const NAVN = mkSong({
  id: "s-navn",
  title: "Navn over alle navn",
  author: "Tore Aas",
  language: "no",
});

const ALL = [AMAZING, NAVN];

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SongsPanel />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.song.list.mockResolvedValue(ALL);
  ipcMock.song.create.mockResolvedValue(mkSong({ id: "s-new", title: "Ny" }));
  ipcMock.song.update.mockResolvedValue(AMAZING);
  ipcMock.song.delete.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("SongsPanel", () => {
  it("lists songs from the catalog", async () => {
    renderPanel();
    expect(screen.getByText("Sangkatalog")).toBeInTheDocument();
    expect(await screen.findByText("Amazing Grace")).toBeInTheDocument();
    expect(screen.getByText("Navn over alle navn")).toBeInTheDocument();
  });

  it("filters the list by title/author search", async () => {
    renderPanel();
    await screen.findByText("Amazing Grace");

    fireEvent.change(screen.getByLabelText("Søk i sanger"), {
      target: { value: "newton" },
    });
    expect(screen.getByText("Amazing Grace")).toBeInTheDocument();
    expect(screen.queryByText("Navn over alle navn")).not.toBeInTheDocument();
  });

  it("creates a song with a trimmed payload", async () => {
    renderPanel();
    await screen.findByText("Amazing Grace");

    fireEvent.click(screen.getByRole("button", { name: "Ny sang" }));
    fireEvent.change(screen.getByPlaceholderText("Navn over alle navn"), {
      target: { value: "  Min sang  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Lagre" }));

    await waitFor(() =>
      expect(ipcMock.song.create).toHaveBeenCalledWith({
        title: "Min sang",
        author: undefined,
        body: undefined,
        language: undefined,
        tonoWorkId: undefined,
      }),
    );
  });

  it("disables save when the title is blank", async () => {
    renderPanel();
    await screen.findByText("Amazing Grace");
    fireEvent.click(screen.getByRole("button", { name: "Ny sang" }));
    expect(screen.getByRole("button", { name: "Lagre" })).toBeDisabled();
  });

  it("edits an existing song via ipc.song.update", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Amazing Grace"));

    const title = screen.getByDisplayValue("Amazing Grace");
    fireEvent.change(title, { target: { value: "Amazing Grace (rev)" } });
    fireEvent.click(screen.getByRole("button", { name: "Lagre" }));

    await waitFor(() =>
      expect(ipcMock.song.update).toHaveBeenCalledWith(
        "s-amazing",
        expect.objectContaining({ title: "Amazing Grace (rev)" }),
      ),
    );
  });

  it("deletes only after the confirm step", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Amazing Grace"));

    // First click reveals the confirm; it does not delete yet.
    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    expect(ipcMock.song.delete).not.toHaveBeenCalled();

    // Confirm.
    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    await waitFor(() =>
      expect(ipcMock.song.delete).toHaveBeenCalledWith("s-amazing"),
    );
  });

  it("can cancel the delete confirmation", async () => {
    renderPanel();
    fireEvent.click(await screen.findByText("Amazing Grace"));

    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    fireEvent.click(screen.getByRole("button", { name: "Avbryt" }));
    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    // Back to the confirm step, not deleted.
    expect(ipcMock.song.delete).not.toHaveBeenCalled();
  });

  it("surfaces a list failure as an error state", async () => {
    ipcMock.song.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPanel();
    await waitFor(() =>
      expect(
        screen.getByText(/Kunne ikke laste sangkatalogen/),
      ).toBeInTheDocument(),
    );
  });
});
