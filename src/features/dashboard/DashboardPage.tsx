/**
 * Dashboard — the Phase 0 "Hello SundayPaper" view.
 *
 * Its job for now is to PROVE the Rust ↔ React IPC bridge works: it calls the
 * `app_info` command and renders the backend's identity. When the card below
 * shows the greeting + version, the roundtrip is confirmed end to end.
 *
 * Later phases replace this with a real dashboard (recent documents, upcoming
 * Sunday, quick actions).
 */

import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, AlertTriangle, Loader2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";

export function DashboardPage() {
  const infoQuery = useQuery({
    queryKey: ["app_info"],
    queryFn: () => ipc.app.info(),
  });

  return (
    <div className="grid h-full place-items-center p-8">
      <div className="w-full max-w-lg">
        <div className="mb-6 text-center">
          <div className="mb-2 text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 0 · Fundament
          </div>
          <h1 className="text-[var(--text-ui-3xl)] font-bold">SundayPaper</h1>
          <p className="mt-2 text-sm text-[var(--color-fg-muted)]">
            Dokument- og trykksak-følgesvennen i Sunday-suiten.
          </p>
        </div>

        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-5 shadow-[var(--shadow-soft)]">
          {infoQuery.isPending ? (
            <Row
              icon={<Loader2 size={16} className="animate-spin" />}
              tone="muted"
            >
              Kobler til backend…
            </Row>
          ) : infoQuery.isError ? (
            <Row icon={<AlertTriangle size={16} />} tone="danger">
              IPC feilet:{" "}
              {infoQuery.error instanceof IPCError
                ? `${infoQuery.error.code} — ${infoQuery.error.message}`
                : String(infoQuery.error)}
            </Row>
          ) : (
            <>
              <Row icon={<CheckCircle2 size={16} />} tone="success">
                {infoQuery.data.greeting}
              </Row>
              <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                <Field label="Versjon" value={`v${infoQuery.data.version}`} />
                <Field label="Tauri" value={infoQuery.data.tauri_version} />
                <Field label="Plattform" value={infoQuery.data.platform} />
                <Field label="Arkitektur" value={infoQuery.data.arch} />
              </dl>
            </>
          )}
        </div>

        <p className="mt-6 text-center text-xs text-[var(--color-fg-muted)]">
          Trykk{" "}
          <kbd className="rounded border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-1.5 py-0.5 font-mono">
            ⌘K
          </kbd>{" "}
          for kommandopaletten.
        </p>
      </div>
    </div>
  );
}

function Row({
  icon,
  tone,
  children,
}: {
  icon: React.ReactNode;
  tone: "muted" | "success" | "danger";
  children: React.ReactNode;
}) {
  const color =
    tone === "success"
      ? "var(--color-success)"
      : tone === "danger"
        ? "var(--color-danger)"
        : "var(--color-fg-muted)";
  return (
    <div className="flex items-center gap-2.5 text-sm" style={{ color }}>
      {icon}
      <span className="text-[var(--color-fg)]">{children}</span>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-[var(--color-fg-muted)]">{label}</dt>
      <dd className="text-right font-mono text-[var(--color-fg)]">{value}</dd>
    </>
  );
}
