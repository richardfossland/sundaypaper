import { useState } from "react";

import { Sidebar, type Route } from "@/components/Sidebar";
import { CommandPalette } from "@/components/CommandPalette";
import { UpdateBanner } from "@/components/UpdateBanner";
import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { DesignPage } from "@/features/design/DesignPage";
import { AssetsPanel } from "@/features/assets/AssetsPanel";
import { SongsPanel } from "@/features/songs/SongsPanel";
import { SangbokPanel } from "@/features/sangbok/SangbokPanel";
import { BuilderPage } from "@/features/builder/BuilderPage";
import { TemplatesPanel } from "@/features/templates/TemplatesPanel";
import { DocTemplatesPanel } from "@/features/doc-templates/DocTemplatesPanel";
import { ImportJobsPanel } from "@/features/import-jobs/ImportJobsPanel";
import { EditorPage } from "@/features/editor/EditorPage";
import { FormsPage } from "@/features/forms/FormsPage";
import { ExportPage } from "@/features/export/ExportPage";
import { SettingsPage } from "@/features/settings/SettingsPage";

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
        ) : route === "design" ? (
          <DesignPage />
        ) : route === "library" ? (
          <AssetsPanel />
        ) : route === "songs" ? (
          <SongsPanel />
        ) : route === "builder" ? (
          <BuilderPage />
        ) : route === "templates" ? (
          <TemplatesPanel />
        ) : route === "doctemplates" ? (
          <DocTemplatesPanel />
        ) : route === "editor" ? (
          <EditorPage />
        ) : route === "forms" ? (
          <FormsPage />
        ) : route === "export" ? (
          <ExportPage />
        ) : route === "splitter" ? (
          <SangbokPanel />
        ) : route === "imports" ? (
          <ImportJobsPanel />
        ) : (
          <SettingsPage />
        )}
      </main>

      <CommandPalette onNavigate={setRoute} />
      <UpdateBanner />
    </div>
  );
}

export default App;
