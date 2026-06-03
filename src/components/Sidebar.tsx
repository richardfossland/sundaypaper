import {
  LayoutDashboard,
  Library,
  LayoutTemplate,
  PenSquare,
  Scissors,
  FileText,
  ClipboardList,
  Download,
  Settings,
  Plus,
} from "lucide-react";

import { cn } from "@/lib/cn";
import { ThemeToggle } from "@/components/ThemeToggle";

type Route =
  | "dashboard"
  | "library"
  | "builder"
  | "templates"
  | "splitter"
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

const NAV_ITEMS: Array<{ id: Route; label: string; icon: typeof Library }> = [
  { id: "dashboard", label: "Dashbord", icon: LayoutDashboard },
  { id: "library", label: "Bibliotek", icon: Library },
  { id: "builder", label: "Bygger", icon: LayoutTemplate },
  { id: "templates", label: "Maler", icon: PenSquare },
  { id: "splitter", label: "Klipper", icon: Scissors },
  { id: "editor", label: "Editor", icon: FileText },
  { id: "forms", label: "Skjema", icon: ClipboardList },
  { id: "export", label: "Eksport", icon: Download },
];

export function Sidebar({ current, onNavigate, onNewDocument }: SidebarProps) {
  return (
    <nav className="flex h-full w-60 flex-col border-r border-[var(--color-border)] bg-[var(--color-bg-elevated)]">
      {/* Brand */}
      <div className="flex items-center gap-2.5 px-4 py-5">
        <div className="grid h-8 w-8 place-items-center rounded-lg bg-[var(--color-brand)] text-[var(--color-accent)] font-bold">
          P
        </div>
        <div className="leading-tight">
          <div className="text-sm font-semibold">SundayPaper</div>
          <div className="text-[10px] text-[var(--color-fg-muted)] uppercase tracking-wider">
            Document & Print
          </div>
        </div>
      </div>

      {/* Nav */}
      <ul className="flex-1 px-2 space-y-0.5">
        {NAV_ITEMS.map((item) => {
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
