/**
 * Settings page (Phase 9) — app preferences, optional API keys and privacy.
 *
 * Three sections:
 *   1. Appearance  — language picker + theme toggle (theme via the local
 *      Zustand store; language persisted to the backend `setting` store).
 *   2. AI / API key — an optional Anthropic key. A "store in system keychain"
 *      checkbox marks intent; keychain storage itself is gated behind Phase 8,
 *      so for now the value lands in the local `setting` store like the rest.
 *   3. Privacy     — opt-in toggles for the cloud features. Default OFF; form
 *      and member data never leaves the machine (see CLAUDE.md).
 *
 * Data flow:
 *   - Reads every key on mount via `ipc.setting.list()` (TanStack Query).
 *   - Writes via `ipc.setting.set(key, value)` and clears via
 *     `ipc.setting.delete(key)` (mutations). The list query re-queries after
 *     each write so the UI reflects the persisted truth.
 *
 * Slots into the "settings" route in App.tsx.
 */

import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Globe,
  KeyRound,
  Loader2,
  Lock,
  Palette,
  ShieldCheck,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { Setting } from "@/lib/bindings";
import { useTheme } from "@/lib/theme";
import { ThemeToggle } from "@/components/ThemeToggle";
import { cn } from "@/lib/cn";
import { SETTING_KEYS } from "./settings-keys";

// ── Language options ──────────────────────────────────────────────────────────
// Matches the suite-wide locale set (see CLAUDE.md "Languages").

const LANGUAGES: Array<{ value: string; label: string }> = [
  { value: "no", label: "Norsk" },
  { value: "en", label: "English" },
  { value: "sv", label: "Svenska" },
  { value: "da", label: "Dansk" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "pl", label: "Polski" },
];

// ── Privacy toggle definitions ────────────────────────────────────────────────

interface ToggleDef {
  key: string;
  label: string;
  description: string;
}

const PRIVACY_TOGGLES: ToggleDef[] = [
  {
    key: SETTING_KEYS.cloudAiEnabled,
    label: "Sky-AI (Claude)",
    description:
      "Tillat at intent→layout og oversettelser sendes til Claude-API-et. Skjema- og medlemsdata sendes aldri.",
  },
  {
    key: SETTING_KEYS.cloudBackupEnabled,
    label: "Sky-sikkerhetskopi",
    description: "Synkroniser maler og ressurser til skyen. Av som standard.",
  },
  {
    key: SETTING_KEYS.telemetryEnabled,
    label: "Anonym bruksstatistikk",
    description: "Del anonyme krasjrapporter for å forbedre appen.",
  },
];

// ── Helpers ─────────────────────────────────────────────────────────────────

/** Build a `key → value` lookup from the flat settings list. */
function toMap(settings: Setting[] | undefined): Record<string, string> {
  const map: Record<string, string> = {};
  for (const s of settings ?? []) map[s.key] = s.value;
  return map;
}

/** A persisted boolean is the literal string "true"; everything else is false. */
function asBool(value: string | undefined): boolean {
  return value === "true";
}

// ── Section shell ─────────────────────────────────────────────────────────────

function Section({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: typeof Globe;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-5 shadow-[var(--shadow-soft)]">
      <div className="mb-4 flex items-start gap-3">
        <div className="mt-0.5 grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-[color-mix(in_oklch,var(--color-accent)_15%,transparent)] text-[var(--color-accent)]">
          <Icon size={18} aria-hidden />
        </div>
        <div>
          <h2 className="text-sm font-semibold">{title}</h2>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            {description}
          </p>
        </div>
      </div>
      <div className="space-y-4">{children}</div>
    </section>
  );
}

// ── Toggle row ──────────────────────────────────────────────────────────────

function Toggle({
  label,
  description,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex-1">
        <p className="text-sm font-medium">{label}</p>
        <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
          {description}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cn(
          "relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-50",
          checked
            ? "bg-[var(--color-accent)]"
            : "bg-[var(--color-bg-surface)] border border-[var(--color-border)]",
        )}
      >
        <span
          className={cn(
            "absolute top-0.5 h-5 w-5 rounded-full bg-white shadow-sm transition-transform",
            checked ? "translate-x-[1.375rem]" : "translate-x-0.5",
          )}
        />
      </button>
    </div>
  );
}

// ── SettingsPage ──────────────────────────────────────────────────────────────

const QUERY_KEY = ["settings"] as const;

export function SettingsPage() {
  const qc = useQueryClient();
  const themeMode = useTheme((s) => s.mode);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [savedKey, setSavedKey] = useState<string | null>(null);

  // ── Query ──────────────────────────────────────────────────────────────────

  const query = useQuery({
    queryKey: QUERY_KEY,
    queryFn: () => ipc.setting.list(),
  });

  const map = useMemo(() => toMap(query.data), [query.data]);

  const invalidate = () => qc.invalidateQueries({ queryKey: QUERY_KEY });

  // ── Mutations ──────────────────────────────────────────────────────────────

  const setMutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      ipc.setting.set(key, value),
    onSuccess: (setting) => {
      setErrorMsg(null);
      setSavedKey(setting.key);
      invalidate();
    },
    onError: (err) => {
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke lagre innstilling",
      );
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (key: string) => ipc.setting.delete(key),
    onSuccess: invalidate,
    onError: (err) => {
      // Deleting an unset key is a NotFound — harmless from the UI's view.
      if (err instanceof IPCError && err.code === "not_found") {
        invalidate();
        return;
      }
      setErrorMsg(
        err instanceof IPCError ? err.message : "Kunne ikke fjerne innstilling",
      );
    },
  });

  const save = (key: string, value: string) =>
    setMutation.mutate({ key, value });

  // Clear the "saved" flash a moment after it appears.
  useEffect(() => {
    if (!savedKey) return;
    const t = setTimeout(() => setSavedKey(null), 1800);
    return () => clearTimeout(t);
  }, [savedKey]);

  // ── Local form state mirrored from the persisted map ─────────────────────────

  const persistedKey = map[SETTING_KEYS.anthropicApiKey] ?? "";
  const [apiKey, setApiKey] = useState(persistedKey);
  const [apiKeyDirty, setApiKeyDirty] = useState(false);
  // Seed the editable field from the persisted value once the load resolves,
  // but never stomp an edit in progress (`apiKeyDirty`). A re-query after a
  // save resets `dirty` so the field tracks the persisted truth again.
  useEffect(() => {
    if (!apiKeyDirty) setApiKey(persistedKey);
  }, [persistedKey, apiKeyDirty]);

  const keychainChecked = asBool(map[SETTING_KEYS.anthropicKeyInKeychain]);
  const locale = map[SETTING_KEYS.locale] ?? "no";

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
        <div>
          <h1 className="text-[var(--text-ui-xl)] font-bold">Innstillinger</h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Språk, tema, API-nøkler og personvern
          </p>
        </div>
        {query.isPending && (
          <Loader2
            size={16}
            className="animate-spin text-[var(--color-fg-muted)]"
          />
        )}
      </header>

      {/* Scroll area */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto flex max-w-2xl flex-col gap-5">
          {/* Error banner */}
          {(errorMsg || query.isError) && (
            <div className="rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-4 py-2.5 text-sm text-[var(--color-danger)]">
              {errorMsg ??
                (query.error instanceof IPCError
                  ? query.error.message
                  : "Kunne ikke laste innstillingene")}
              {errorMsg && (
                <button
                  type="button"
                  className="ml-3 text-xs underline"
                  onClick={() => setErrorMsg(null)}
                >
                  Lukk
                </button>
              )}
            </div>
          )}

          {/* ── 1. Appearance ──────────────────────────────────────────────── */}
          <Section
            icon={Palette}
            title="Utseende"
            description="Velg språk og fargetema for appen."
          >
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Globe size={14} aria-hidden /> Språk
              </div>
              <select
                aria-label="Språk"
                value={locale}
                onChange={(e) => save(SETTING_KEYS.locale, e.target.value)}
                className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 text-sm outline-none focus:border-[var(--color-accent)]"
              >
                {LANGUAGES.map((l) => (
                  <option key={l.value} value={l.value}>
                    {l.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="flex items-center justify-between gap-4">
              <div className="text-sm font-medium">Tema</div>
              <ThemeToggle />
            </div>
            <p className="text-[11px] text-[var(--color-fg-muted)]">
              Tema lagres lokalt på maskinen ({themeMode}).
            </p>
          </Section>

          {/* ── 2. AI / API key ────────────────────────────────────────────── */}
          <Section
            icon={KeyRound}
            title="AI og API-nøkkel"
            description="En valgfri Anthropic-nøkkel åpner intent→layout og oversettelser."
          >
            <form
              className="flex flex-col gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                const trimmed = apiKey.trim();
                setApiKeyDirty(false);
                if (trimmed) save(SETTING_KEYS.anthropicApiKey, trimmed);
                else deleteMutation.mutate(SETTING_KEYS.anthropicApiKey);
              }}
            >
              <label
                htmlFor="anthropic-api-key"
                className="text-xs text-[var(--color-fg-muted)]"
              >
                Anthropic API-nøkkel
              </label>
              <div className="flex gap-2">
                <input
                  id="anthropic-api-key"
                  type="password"
                  autoComplete="off"
                  value={apiKey}
                  onChange={(e) => {
                    setApiKeyDirty(true);
                    setApiKey(e.target.value);
                  }}
                  placeholder="sk-ant-…"
                  className="flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-1.5 font-mono text-sm outline-none focus:border-[var(--color-accent)]"
                />
                <button
                  type="submit"
                  disabled={setMutation.isPending}
                  className="flex items-center justify-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-xs font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
                >
                  {setMutation.isPending ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : null}
                  Lagre
                </button>
              </div>

              <label className="mt-1 flex items-center gap-2 text-xs text-[var(--color-fg-muted)]">
                <input
                  type="checkbox"
                  checked={keychainChecked}
                  onChange={(e) =>
                    save(
                      SETTING_KEYS.anthropicKeyInKeychain,
                      String(e.target.checked),
                    )
                  }
                  className="accent-[var(--color-accent)]"
                />
                <Lock size={12} aria-hidden />
                Lagre nøkkelen i systemets nøkkelring (Phase 8)
              </label>
              {savedKey === SETTING_KEYS.anthropicApiKey && (
                <p className="text-[11px] text-[var(--color-accent)]">
                  Lagret.
                </p>
              )}
            </form>
          </Section>

          {/* ── 3. Privacy ─────────────────────────────────────────────────── */}
          <Section
            icon={ShieldCheck}
            title="Personvern"
            description="Skytjenester er valgfrie og av som standard. Skjema- og medlemsdata forlater aldri maskinen."
          >
            {PRIVACY_TOGGLES.map((t) => (
              <Toggle
                key={t.key}
                label={t.label}
                description={t.description}
                checked={asBool(map[t.key])}
                disabled={setMutation.isPending}
                onChange={(next) => save(t.key, String(next))}
              />
            ))}
          </Section>
        </div>
      </div>
    </div>
  );
}
