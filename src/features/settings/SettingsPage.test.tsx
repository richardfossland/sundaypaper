/// <reference types="@testing-library/jest-dom/vitest" />
/**
 * SettingsPage unit tests — drive the preferences page with `@/lib/ipc` mocked
 * so the list query and the set/delete mutations run without a backend, DB or
 * keychain. Mirrors the seams in AssetsPanel.test.tsx (whole-module ipc mock +
 * a real-ish IPCError so `instanceof` holds).
 *
 * Coverage: loads + reflects persisted settings, the language picker writes
 * `locale`, the API-key form roundtrips (set when filled / delete when cleared),
 * the keychain checkbox persists, the privacy toggles flip, and a list failure
 * surfaces an error.
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

import type { Setting } from "@/lib/bindings";

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
      setting: {
        list: vi.fn(),
        get: vi.fn(),
        set: vi.fn(),
        delete: vi.fn(),
      },
    },
  };
});

vi.mock("@/lib/ipc", () => ({
  ipc: ipcMock,
  IPCError: FakeIPCError,
}));

import { SettingsPage } from "./SettingsPage";
import { SETTING_KEYS } from "./settings-keys";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const mkSetting = (key: string, value: string): Setting => ({
  key,
  value,
  updated_at: 0n,
});

const SETTINGS: Setting[] = [
  mkSetting(SETTING_KEYS.locale, "en"),
  mkSetting(SETTING_KEYS.anthropicApiKey, "sk-ant-existing"),
  mkSetting(SETTING_KEYS.cloudAiEnabled, "true"),
];

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsPage />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  ipcMock.setting.list.mockResolvedValue(SETTINGS);
  ipcMock.setting.set.mockImplementation((key: string, value: string) =>
    Promise.resolve(mkSetting(key, value)),
  );
  ipcMock.setting.delete.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("SettingsPage", () => {
  it("renders the three sections and lists settings on mount", async () => {
    renderPage();
    expect(screen.getByText("Innstillinger")).toBeInTheDocument();
    expect(screen.getByText("Utseende")).toBeInTheDocument();
    expect(screen.getByText("AI og API-nøkkel")).toBeInTheDocument();
    expect(screen.getByText("Personvern")).toBeInTheDocument();
    await waitFor(() => expect(ipcMock.setting.list).toHaveBeenCalled());
  });

  it("reflects the persisted locale + api key once the query resolves", async () => {
    renderPage();
    // Persisted locale "en" should be the selected option.
    await waitFor(() =>
      expect((screen.getByLabelText("Språk") as HTMLSelectElement).value).toBe(
        "en",
      ),
    );
    expect(
      (screen.getByLabelText("Anthropic API-nøkkel") as HTMLInputElement).value,
    ).toBe("sk-ant-existing");
  });

  it("writes the chosen language via setting.set('locale', …)", async () => {
    renderPage();
    await screen.findByText("Utseende");

    fireEvent.change(screen.getByLabelText("Språk"), {
      target: { value: "no" },
    });
    await waitFor(() =>
      expect(ipcMock.setting.set).toHaveBeenCalledWith(
        SETTING_KEYS.locale,
        "no",
      ),
    );
  });

  it("roundtrips the API key: set when filled", async () => {
    renderPage();
    await screen.findByText("AI og API-nøkkel");

    const input = screen.getByLabelText("Anthropic API-nøkkel");
    fireEvent.change(input, { target: { value: "  sk-ant-new  " } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() =>
      // The value is trimmed before it hits the backend.
      expect(ipcMock.setting.set).toHaveBeenCalledWith(
        SETTING_KEYS.anthropicApiKey,
        "sk-ant-new",
      ),
    );
    // A "saved" confirmation appears.
    expect(await screen.findByText("Lagret.")).toBeInTheDocument();
  });

  it("roundtrips the API key: delete when cleared", async () => {
    renderPage();
    const input = await screen.findByLabelText("Anthropic API-nøkkel");

    fireEvent.change(input, { target: { value: "" } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() =>
      expect(ipcMock.setting.delete).toHaveBeenCalledWith(
        SETTING_KEYS.anthropicApiKey,
      ),
    );
    expect(ipcMock.setting.set).not.toHaveBeenCalledWith(
      SETTING_KEYS.anthropicApiKey,
      expect.anything(),
    );
  });

  it("persists the keychain checkbox as a boolean string", async () => {
    renderPage();
    await screen.findByText("AI og API-nøkkel");

    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /nøkkelring/i,
      }),
    );
    await waitFor(() =>
      expect(ipcMock.setting.set).toHaveBeenCalledWith(
        SETTING_KEYS.anthropicKeyInKeychain,
        "true",
      ),
    );
  });

  it("reflects a persisted privacy toggle as on, and flips it off", async () => {
    renderPage();
    // cloud_ai_enabled is "true" in the fixture.
    const toggle = await screen.findByRole("switch", {
      name: "Sky-AI (Claude)",
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));

    fireEvent.click(toggle);
    await waitFor(() =>
      expect(ipcMock.setting.set).toHaveBeenCalledWith(
        SETTING_KEYS.cloudAiEnabled,
        "false",
      ),
    );
  });

  it("turns an off-by-default privacy toggle on", async () => {
    renderPage();
    const toggle = await screen.findByRole("switch", {
      name: "Sky-sikkerhetskopi",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    fireEvent.click(toggle);
    await waitFor(() =>
      expect(ipcMock.setting.set).toHaveBeenCalledWith(
        SETTING_KEYS.cloudBackupEnabled,
        "true",
      ),
    );
  });

  it("surfaces a list failure as an error banner", async () => {
    ipcMock.setting.list.mockRejectedValue(
      new FakeIPCError("internal", "db gone"),
    );
    renderPage();
    expect(await screen.findByText("db gone")).toBeInTheDocument();
  });

  it("treats a delete NotFound as harmless (no error banner)", async () => {
    ipcMock.setting.list.mockResolvedValue([
      mkSetting(SETTING_KEYS.locale, "no"),
    ]);
    ipcMock.setting.delete.mockRejectedValue(
      new FakeIPCError("not_found", "setting"),
    );
    renderPage();
    const input = await screen.findByLabelText("Anthropic API-nøkkel");

    fireEvent.change(input, { target: { value: "" } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() =>
      expect(ipcMock.setting.delete).toHaveBeenCalledWith(
        SETTING_KEYS.anthropicApiKey,
      ),
    );
    // No error surfaced for the benign NotFound.
    expect(
      screen.queryByText("Kunne ikke fjerne innstilling"),
    ).not.toBeInTheDocument();
  });
});
