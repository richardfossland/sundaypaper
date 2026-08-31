/**
 * Auto-update banner.
 *
 * On launch, checks the GitHub Releases manifest for a newer signed build. If
 * one exists, offers a one-click download + relaunch. No-ops silently outside
 * Tauri / offline / before the first release, so it never gets in the way.
 */
import { useEffect, useState } from "react";
import { Download, X } from "lucide-react";

import { checkForUpdate, installAndRelaunch, type Update } from "@/lib/updater";

export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [busy, setBusy] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    checkForUpdate()
      .then((u) => u && setUpdate(u))
      .catch(() => {});
  }, []);

  if (!update || dismissed) return null;

  return (
    <div className="fixed right-4 bottom-4 z-50 w-[min(92vw,420px)] rounded-xl border border-[var(--color-accent)]/40 bg-[var(--color-bg-elevated)] p-4 shadow-lg">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-semibold">
            Oppdatering tilgjengelig
            {update.version ? ` (${update.version})` : ""}
          </p>
          {/* HVA som er nytt — fra manifestets `notes`, som siden
              `docs/release-notes/<tagg>.md` er en tekst et menneske har skrevet
              til brukeren og ikke en fast setning fra byggefila. Tillegget har
              båret `body` hele veien hit; banneret kastet det og skrev bare
              versjonsnummeret, så hver oppdatering så lik ut som forrige.

              Ren tekst med bevarte linjeskift, ikke markdown — vakten i
              `scripts/release-notes.mjs` avviser markdown i notatet nettopp
              fordi denne boksen ikke har noen renderer å vise det med. */}
          {update.body?.trim() ? (
            <p className="mt-2 max-h-48 overflow-y-auto text-xs whitespace-pre-line text-[var(--color-fg)]">
              {update.body.trim()}
            </p>
          ) : (
            <p className="mt-1 text-xs text-[var(--color-fg-muted)]">
              En nyere versjon av SundayPaper er klar. Last ned og start på nytt
              for å oppdatere.
            </p>
          )}
        </div>
        <button
          type="button"
          aria-label="Lukk"
          onClick={() => setDismissed(true)}
          className="rounded-md p-1 text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)]"
        >
          <X size={16} />
        </button>
      </div>
      <div className="mt-3 flex justify-end">
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            installAndRelaunch(update).catch(() => setBusy(false));
          }}
          className="flex items-center gap-2 rounded-md bg-[var(--color-accent)] px-4 py-1.5 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-60"
        >
          <Download size={14} />
          {busy ? "Oppdaterer …" : "Last ned"}
        </button>
      </div>
    </div>
  );
}
