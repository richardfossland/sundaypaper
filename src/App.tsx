import { useState } from "react";

import { Sidebar, type Route } from "@/components/Sidebar";
import { CommandPalette } from "@/components/CommandPalette";
import { DashboardPage } from "@/features/dashboard/DashboardPage";

function App() {
  const [route, setRoute] = useState<Route>("dashboard");

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--color-bg)] text-[var(--color-fg)]">
      <Sidebar
        current={route}
        onNavigate={setRoute}
        onNewDocument={() => setRoute("builder")}
      />

      <main className="flex-1 overflow-hidden">
        {route === "dashboard" ? (
          <DashboardPage />
        ) : (
          <Placeholder route={route} />
        )}
      </main>

      <CommandPalette onNavigate={setRoute} />
    </div>
  );
}

function Placeholder({ route }: { route: Exclude<Route, "dashboard"> }) {
  const titles: Record<Exclude<Route, "dashboard">, { title: string; phase: string }> = {
    library:  { title: "Ressursbibliotek", phase: "Phase 2.3" },
    builder:  { title: "Dokumentbygger",   phase: "Phase 4.3" },
    splitter: { title: "Sangbok-klipper",  phase: "Phase 3" },
    editor:   { title: "PDF-editor",       phase: "Phase 7.1" },
    forms:    { title: "Skjema",           phase: "Phase 7.2" },
    export:   { title: "Eksport",          phase: "Phase 6" },
    settings: { title: "Innstillinger",    phase: "Phase 9" },
  };
  const info = titles[route];

  return (
    <div className="grid h-full place-items-center">
      <div className="max-w-sm text-center">
        <div className="mb-2 text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
          {info.phase}
        </div>
        <h1 className="mb-2 text-[var(--text-ui-2xl)] font-bold">{info.title}</h1>
        <p className="text-sm text-[var(--color-fg-muted)]">
          Denne siden er planlagt for {info.phase}. Scaffolding er på plass —
          implementasjon kommer i senere fase.
        </p>
        <p className="mt-6 text-xs text-[var(--color-fg-muted)]">
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

export default App;
