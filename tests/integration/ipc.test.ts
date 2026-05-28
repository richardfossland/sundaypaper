// Integration smoke — the IPC client layer. Mocks Tauri's `invoke` for the
// happy path, and tests the AppError -> IPCError mapping via the pure
// `toIPCError` helper (no async rejection plumbing required).
import { describe, it, expect, vi, beforeEach } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { ipc, toIPCError, IPCError } from "@/lib/ipc";

describe("ipc client", () => {
  beforeEach(() => invokeMock.mockReset());

  it("calls the named command and returns its result", async () => {
    invokeMock.mockResolvedValue({
      name: "SundayPaper",
      version: "0.1.0",
      tauri_version: "2",
      platform: "macos",
      arch: "aarch64",
      greeting: "hi",
    });
    const info = await ipc.app.info();
    expect(info.name).toBe("SundayPaper");
    expect(invokeMock).toHaveBeenCalledWith("app_info", undefined);
  });
});

describe("toIPCError", () => {
  it("maps a serialised AppError to an IPCError preserving `code`", () => {
    const err = toIPCError({ code: "not_found", message: "nope" });
    expect(err).toBeInstanceOf(IPCError);
    expect(err).toMatchObject({
      name: "IPCError",
      code: "not_found",
      message: "nope",
    });
  });

  it("passes through a real Error unchanged", () => {
    const original = new Error("boom");
    expect(toIPCError(original)).toBe(original);
  });

  it("wraps an unknown value as a generic Error", () => {
    const err = toIPCError("weird");
    expect(err).toBeInstanceOf(Error);
    expect(err.message).toBe("weird");
  });
});
