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
    ipcMock: { importJob: { list: vi.fn() } },
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

  it("hides finished jobs when the filter is toggled", async () => {
    renderPanel();
    await screen.findByText("bulletin.pdf");

    fireEvent.click(screen.getByLabelText("Skjul ferdige"));
    // Both DONE and FAILED are terminal, so the list empties.
    expect(screen.queryByText("bulletin.pdf")).not.toBeInTheDocument();
    expect(screen.queryByText("broken.pdf")).not.toBeInTheDocument();
    expect(screen.getByText(/Ingen jobber å vise/)).toBeInTheDocument();
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
