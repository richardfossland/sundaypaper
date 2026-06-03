/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * AssetsPanel unit tests — drive the asset-library browser with `@/lib/ipc`
 * mocked so the list query and the add/delete/open mutations run without a
 * backend, DB, or filesystem. Mirrors the seams used in EditorPage.test.tsx
 * (whole-module ipc mock + a real-ish IPCError class so `instanceof` holds).
 *
 * Coverage: grid renders assets, kind filter re-queries, client-side search
 * over name + tags, clickable tag filter chips, the drag-drop → add flow, and
 * the delete / open mutations.
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

import type { AssetLibEntry } from "@/lib/bindings";

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
      assetLib: {
        list: vi.fn(),
        add: vi.fn(),
        delete: vi.fn(),
        open: vi.fn(),
      },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { AssetsPanel } from "./AssetsPanel";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const mkAsset = (over: Partial<AssetLibEntry>): AssetLibEntry => ({
  id: "a-x",
  name: "Asset",
  kind: "Logo",
  file_path: "/tmp/asset.png",
  tags: "",
  created_at: 0n,
  ...over,
});

const LOGO = mkAsset({
  id: "a-logo",
  name: "Menighetslogo",
  kind: "Logo",
  file_path: "/tmp/logo.png",
  tags: "brand, 2026",
});
const TEMPLATE = mkAsset({
  id: "a-tpl",
  name: "Høymesse-mal",
  kind: "Template",
  file_path: "/tmp/mal.typ",
  tags: "bulletin, 2026",
});
const SONG = mkAsset({
  id: "a-song",
  name: "Navn over alle navn",
  kind: "SongSheet",
  file_path: "/tmp/song.pdf",
  tags: "",
});

const ALL = [LOGO, TEMPLATE, SONG];

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <AssetsPanel />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.assetLib.list.mockResolvedValue(ALL);
  ipcMock.assetLib.add.mockResolvedValue(mkAsset({ id: "a-new" }));
  ipcMock.assetLib.delete.mockResolvedValue(undefined);
  ipcMock.assetLib.open.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("AssetsPanel", () => {
  it("renders the panel shell and lists assets in a grid", async () => {
    renderPanel();
    expect(screen.getByText("Ressursbibliotek")).toBeInTheDocument();
    expect(await screen.findByText("Menighetslogo")).toBeInTheDocument();
    expect(screen.getByText("Høymesse-mal")).toBeInTheDocument();
    expect(screen.getByText("Navn over alle navn")).toBeInTheDocument();
    // "all" kind → list called with undefined.
    expect(ipcMock.assetLib.list).toHaveBeenCalledWith(undefined);
  });

  it("re-queries with the selected kind when a kind filter is chosen", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.click(screen.getByRole("button", { name: "Maler" }));
    await waitFor(() =>
      expect(ipcMock.assetLib.list).toHaveBeenCalledWith("Template"),
    );
  });

  it("filters client-side via the search box (name + tags)", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.change(screen.getByLabelText("Søk i ressurser"), {
      target: { value: "navn" },
    });

    // Only the song matches "navn"; logo + template drop out.
    expect(screen.getByText("Navn over alle navn")).toBeInTheDocument();
    expect(screen.queryByText("Menighetslogo")).not.toBeInTheDocument();
    expect(screen.queryByText("Høymesse-mal")).not.toBeInTheDocument();
  });

  it("matches assets by tag text in the search box", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.change(screen.getByLabelText("Søk i ressurser"), {
      target: { value: "brand" },
    });
    expect(screen.getByText("Menighetslogo")).toBeInTheDocument();
    expect(screen.queryByText("Høymesse-mal")).not.toBeInTheDocument();
  });

  it("pins a single tag via the tag-filter chips", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    // The "bulletin" tag only lives on the template.
    fireEvent.click(screen.getByRole("button", { name: "#bulletin" }));
    expect(screen.getByText("Høymesse-mal")).toBeInTheDocument();
    expect(screen.queryByText("Menighetslogo")).not.toBeInTheDocument();
    expect(screen.queryByText("Navn over alle navn")).not.toBeInTheDocument();
  });

  it("shows a no-results state with a reset when a search matches nothing", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.change(screen.getByLabelText("Søk i ressurser"), {
      target: { value: "zzz-no-match" },
    });
    expect(screen.getByText("Ingen treff for søket.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Tøm filter" }));
    expect(await screen.findByText("Menighetslogo")).toBeInTheDocument();
  });

  it("deletes an asset via ipc.assetLib.delete", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.click(
      screen.getByRole("button", { name: "Slett Menighetslogo" }),
    );
    await waitFor(() =>
      expect(ipcMock.assetLib.delete).toHaveBeenCalledWith("a-logo"),
    );
  });

  it("opens an asset via ipc.assetLib.open", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.click(screen.getByRole("button", { name: "Åpne Menighetslogo" }));
    await waitFor(() =>
      expect(ipcMock.assetLib.open).toHaveBeenCalledWith("a-logo"),
    );
  });

  it("registers a dropped file: pre-fills name + guessed kind, then adds it", async () => {
    renderPanel();
    await screen.findByText("Menighetslogo");

    const file = new File(["data"], "ny-logo.png", { type: "image/png" });
    // Tauri carries the absolute path on the (non-standard) File.path field.
    Object.defineProperty(file, "path", { value: "/tmp/ny-logo.png" });
    const dropZone = screen.getByText(/Dra filer hit/).closest("div")!;

    fireEvent.drop(dropZone, { dataTransfer: { files: [file] } });

    // The inline add form appears with the basename pre-filled.
    const nameInput = (await screen.findByPlaceholderText(
      "Navn på asset …",
    )) as HTMLInputElement;
    expect(nameInput.value).toBe("ny-logo");

    fireEvent.change(screen.getByPlaceholderText("Tagger (komma-separert) …"), {
      target: { value: "brand" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Legg til" }));

    await waitFor(() =>
      expect(ipcMock.assetLib.add).toHaveBeenCalledWith({
        name: "ny-logo",
        kind: "Logo",
        filePath: "/tmp/ny-logo.png",
        tags: "brand",
      }),
    );
  });

  it("surfaces a list failure as an error state", async () => {
    ipcMock.assetLib.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPanel();

    await waitFor(() =>
      expect(
        screen.getByText(/Kunne ikke laste biblioteket/),
      ).toBeInTheDocument(),
    );
  });

  it("surfaces a delete failure as an error banner", async () => {
    ipcMock.assetLib.delete.mockRejectedValue(
      new FakeIPCError("io", "file locked"),
    );
    renderPanel();
    await screen.findByText("Menighetslogo");

    fireEvent.click(
      screen.getByRole("button", { name: "Slett Menighetslogo" }),
    );
    const banner = await screen.findByText("file locked");
    expect(banner).toBeInTheDocument();
    // The asset is still present (delete did not invalidate to empty).
    expect(
      within(document.body).getByText("Menighetslogo"),
    ).toBeInTheDocument();
  });
});
