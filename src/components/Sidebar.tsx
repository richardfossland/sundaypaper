import {
  LayoutDashboard,
  Library,
  Music,
  LayoutTemplate,
  PenSquare,
  FileStack,
  Scissors,
  FileText,
  ClipboardList,
  Download,
  History,
  Settings,
  Plus,
} from "lucide-react";

import { cn } from "@/lib/cn";
import { ThemeToggle } from "@/components/ThemeToggle";
import logoUrl from "@/assets/logo.svg";

type Route =
  | "dashboard"
  | "library"
  | "songs"
  | "builder"
  | "templates"
  | "doctemplates"
  | "splitter"
  | "imports"
  | "editor"
  | "forms"
  | "export"
  | "settings"
  | "design";

interface SidebarProps {
  current: Route;
  onNavigate: (route: Route) => void;
  onNewDocument: () => void;
}

type NavItem = { id: Route; label: string; icon: typeof Library };

// Eleven flat items read as a wall — group them by intent so the rail orients
// at a glance: a top Dashboard, then Lag / Innhold / Arkiv (each ≤4 items).
const NAV_SECTIONS: Array<{ heading: string | null; items: NavItem[] }> = [
  {
    heading: null,
    items: [{ id: "dashboard", label: "Dashbord", icon: LayoutDashboard }],
  },
  {
    heading: "Lag",
    items: [
      { id: "builder", label: "Bygger", icon: LayoutTemplate },
      { id: "editor", label: "Editor", icon: FileText },
      { id: "splitter", label: "Klipper", icon: Scissors },
      { id: "forms", label: "Skjema", icon: ClipboardList },
    ],
  },
  {
    heading: "Innhold",
    items: [
      { id: "library", label: "Bibliotek", icon: Library },
      { id: "songs", label: "Sanger", icon: Music },
      { id: "templates", label: "Maler", icon: PenSquare },
      { id: "doctemplates", label: "Dokumentmaler", icon: FileStack },
    ],
  },
  {
    heading: "Arkiv",
    items: [
      { id: "imports", label: "Importlogg", icon: History },
      { id: "export", label: "Eksport", icon: Download },
    ],
  },
];

export function Sidebar({ current, onNavigate, onNewDocument }: SidebarProps) {
  return (
    <nav className="flex h-full w-60 flex-col border-r border-[var(--color-border)] bg-[var(--color-bg-elevated)]">
      {/* Brand */}
      <div className="flex items-center gap-2.5 px-4 py-5">
        <img
          src={logoUrl}
          width={32}
          height={32}
          alt=""
          aria-hidden="true"
          className="block rounded-[22%]"
        />
        <div className="leading-tight">
          <div className="text-sm font-semibold">SundayPaper</div>
          <div className="text-[10px] text-[var(--color-fg-muted)] uppercase tracking-wider">
            Document &amp; Print
          </div>
        </div>
      </div>

      {/* Nav */}
      <div className="flex-1 overflow-y-auto px-2">
        {NAV_SECTIONS.map((section) => (
          <div
            key={section.heading ?? "top"}
            className={section.heading ? "mt-4" : ""}
          >
            {section.heading ? (
              <div className="px-3 pb-1 text-[10px] font-medium uppercase tracking-wider text-[var(--color-fg-muted)]">
                {section.heading}
              </div>
            ) : null}
            <ul className="space-y-0.5">
              {section.items.map((item) => {
                const Icon = item.icon;
                const isActive = current === item.id;
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      onClick={() => onNavigate(item.id)}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                        isActive
                          ? "bg-[var(--color-bg-surface)] text-[var(--color-fg)]"
                          : "text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]/60 hover:text-[var(--color-fg)]",
                      )}
                    >
                      <Icon size={16} aria-hidden />
                      <span>{item.label}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>

      {/* Bottom */}
      <div className="space-y-2 border-t border-[var(--color-border)] p-3">
        <div className="flex items-center justify-between px-1 pb-1">
          <span className="text-[10px] text-[var(--color-fg-muted)] uppercase tracking-wider">
            Tema
          </span>
          <ThemeToggle />
        </div>
        <button
          type="button"
          onClick={() => onNavigate("settings")}
          className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]/60 hover:text-[var(--color-fg)]"
        >
          <Settings size={16} aria-hidden />
          <span>Innstillinger</span>
        </button>
        <button
          type="button"
          onClick={onNewDocument}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-[var(--color-accent)] px-3 py-2.5 text-sm font-bold text-[var(--color-accent-fg)] shadow-sm transition-all hover:brightness-110 active:translate-y-px"
        >
          <Plus size={16} aria-hidden />
          <span>Nytt dokument</span>
        </button>
      </div>
    </nav>
  );
}

export type { Route };
