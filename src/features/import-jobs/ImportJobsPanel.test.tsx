/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * ImportJobsPanel unit tests — drive the read-only import-job history UI with
 * `@/lib/ipc` mocked. Mirrors the SongsPanel.test.tsx seams.
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

import type { ImportJob } from "@/lib/bindings";

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
      importJob: {
        list: vi.fn(),
        delete: vi.fn(),
        clearFinished: vi.fn(),
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

import { ImportJobsPanel } from "./ImportJobsPanel";

const mkJob = (over: Partial<ImportJob>): ImportJob => ({
  id: "j-x",
  project_id: null,
  source_path: "/tmp/scan.pdf",
  kind: "ocr",
  status: "pending",
  detail: null,
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

const DONE = mkJob({
  id: "j-done",
  source_path: "/scans/bulletin.pdf",
  status: "done",
  created_at: 100n,
});
const FAILED = mkJob({
  id: "j-failed",
  source_path: "/scans/broken.pdf",
  status: "error",
  detail: "tesseract crashed on page 3",
  created_at: 200n,
});

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <ImportJobsPanel />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.importJob.list.mockResolvedValue([DONE, FAILED]);
  ipcMock.importJob.delete.mockResolvedValue(undefined);
  ipcMock.importJob.clearFinished.mockResolvedValue(2);
});

afterEach(() => cleanup());

describe("ImportJobsPanel", () => {
  it("lists past jobs with status badges and file names", async () => {
    renderPanel();
    expect(screen.getByText("Importlogg")).toBeInTheDocument();
    expect(await screen.findByText("bulletin.pdf")).toBeInTheDocument();
    expect(screen.getByText("broken.pdf")).toBeInTheDocument();
    expect(screen.getByText("Ferdig")).toBeInTheDocument();
    expect(screen.getByText("Feilet")).toBeInTheDocument();
  });

  it("shows the error detail for a failed job", async () => {
    renderPanel();
    expect(
      await screen.findByText("tesseract crashed on page 3"),
    ).toBeInTheDocument();
  });

  it("deletes a single job only after the confirm step", async () => {
    renderPanel();
    await screen.findByText("bulletin.pdf");

    // First click on the row's trash icon reveals the confirm step.
    fireEvent.click(screen.getByLabelText("Slett bulletin.pdf"));
    expect(ipcMock.importJob.delete).not.toHaveBeenCalled();

    // Confirm actually deletes.
    fireEvent.click(screen.getByRole("button", { name: "Slett" }));
    await waitFor(() =>
      expect(ipcMock.importJob.delete).toHaveBeenCalledWith("j-done"),
    );
  });

  it("clears finished jobs only after the confirm step", async () => {
    renderPanel();
    await screen.findByText("bulletin.pdf");

    fireEvent.click(screen.getByRole("button", { name: "Tøm ferdige" }));
    expect(ipcMock.importJob.clearFinished).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Tøm" }));
    await waitFor(() =>
      expect(ipcMock.importJob.clearFinished).toHaveBeenCalledTimes(1),
    );
  });

  it("hides the clear action when there are no finished jobs", async () => {
    ipcMock.importJob.list.mockResolvedValue([
      mkJob({ id: "j-run", status: "running" }),
    ]);
    renderPanel();
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Tøm ferdige" }),
      ).not.toBeInTheDocument(),
    );
  });

  it("shows an empty state when there are no jobs", async () => {
    ipcMock.importJob.list.mockResolvedValue([]);
    renderPanel();
    await waitFor(() =>
      expect(screen.getByText(/Ingen importjobber ennå/)).toBeInTheDocument(),
    );
  });

  it("surfaces a list failure as an error state", async () => {
    ipcMock.importJob.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPanel();
    await waitFor(() =>
      expect(
        screen.getByText(/Kunne ikke laste importloggen/),
      ).toBeInTheDocument(),
    );
  });
});
