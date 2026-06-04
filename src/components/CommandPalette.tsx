/**
 * ⌘K command palette — keyboard-first navigation + actions.
 *
 * Phase 0 surfaces page navigation and a couple of stub actions. Later phases
 * feed in search results (assets, documents, songs) and quick-insert from the
 * asset library.
 */

import { Command } from "cmdk";
import { useEffect, useState } from "react";
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
  Settings,
  Plus,
  Wand2,
  Palette,
} from "lucide-react";

import type { Route } from "./Sidebar";

interface CommandPaletteProps {
  onNavigate: (route: Route) => void;
}

export function CommandPalette({ onNavigate }: CommandPaletteProps) {
  const [open, setOpen] = useState(false);

  // ⌘K / Ctrl+K toggles the palette
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  function go(route: Route) {
    onNavigate(route);
    setOpen(false);
  }

  if (!open) return null;

  return (
    <Command.Dialog
      open
      onOpenChange={setOpen}
      label="Kommandopalett"
      className="fixed inset-0 z-50 grid place-items-start pt-[12vh]"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={() => setOpen(false)}
        aria-hidden
      />

      <div className="relative w-full max-w-2xl mx-auto overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-[var(--shadow-elevated)]">
        <Command.Input
          autoFocus
          placeholder="Søk etter dokumenter, ressurser, eller skriv en kommando…"
          className="w-full border-b border-[var(--color-border)] bg-transparent px-4 py-3 text-[var(--text-ui-md)] text-[var(--color-fg)] placeholder:text-[var(--color-fg-muted)] focus:outline-none"
        />
        <Command.List className="max-h-[60vh] overflow-y-auto p-2">
          <Command.Empty className="px-3 py-6 text-center text-sm text-[var(--color-fg-muted)]">
            Ingen treff.
          </Command.Empty>

          <Command.Group
            heading="Naviger"
            className="text-xs font-medium uppercase tracking-wider text-[var(--color-fg-muted)] mb-1 mt-2 px-2"
          >
            <Item
              onSelect={() => go("dashboard")}
              icon={<LayoutDashboard size={14} />}
              label="Dashbord"
            />
            <Item
              onSelect={() => go("library")}
              icon={<Library size={14} />}
              label="Bibliotek"
            />
            <Item
              onSelect={() => go("songs")}
              icon={<Music size={14} />}
              label="Sanger"
            />
            <Item
              onSelect={() => go("builder")}
              icon={<LayoutTemplate size={14} />}
              label="Bygger"
            />
            <Item
              onSelect={() => go("templates")}
              icon={<PenSquare size={14} />}
              label="Maler"
            />
            <Item
              onSelect={() => go("doctemplates")}
              icon={<FileStack size={14} />}
              label="Dokumentmaler"
            />
            <Item
              onSelect={() => go("splitter")}
              icon={<Scissors size={14} />}
              label="Klipper"
            />
            <Item
              onSelect={() => go("editor")}
              icon={<FileText size={14} />}
              label="Editor"
            />
            <Item
              onSelect={() => go("forms")}
              icon={<ClipboardList size={14} />}
              label="Skjema"
            />
            <Item
              onSelect={() => go("export")}
              icon={<Download size={14} />}
              label="Eksport"
            />
            <Item
              onSelect={() => go("settings")}
              icon={<Settings size={14} />}
              label="Innstillinger"
            />
          </Command.Group>

          <Command.Group
            heading="Handlinger"
            className="text-xs font-medium uppercase tracking-wider text-[var(--color-fg-muted)] mb-1 mt-4 px-2"
          >
            <Item
              onSelect={() => go("builder")}
              icon={<Plus size={14} />}
              label="Nytt dokument…"
              shortcut="⌘N"
            />
            <Item
              onSelect={() => go("builder")}
              icon={<Wand2 size={14} />}
              label="Lag program fra plan…"
              shortcut="⌘G"
            />
            <Item
              onSelect={() => go("splitter")}
              icon={<Scissors size={14} />}
              label="Klipp sangbok…"
            />
          </Command.Group>

          {import.meta.env.DEV && (
            <Command.Group
              heading="Utvikler"
              className="mt-4 mb-1 px-2 text-xs font-medium tracking-wider text-[var(--color-fg-muted)] uppercase"
            >
              <Item
                onSelect={() => go("design")}
                icon={<Palette size={14} />}
                label="Designsystem"
              />
            </Command.Group>
          )}
        </Command.List>
      </div>
    </Command.Dialog>
  );
}

function Item({
  onSelect,
  icon,
  label,
  shortcut,
}: {
  onSelect: () => void;
  icon: React.ReactNode;
  label: string;
  shortcut?: string;
}) {
  return (
    <Command.Item
      onSelect={onSelect}
      className="flex cursor-pointer items-center gap-2.5 rounded-md px-3 py-2 text-sm text-[var(--color-fg)] aria-selected:bg-[var(--color-bg-surface)]"
    >
      <span className="text-[var(--color-fg-muted)]">{icon}</span>
      <span className="flex-1">{label}</span>
      {shortcut ? (
        <kbd className="rounded border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--color-fg-muted)]">
          {shortcut}
        </kbd>
      ) : null}
    </Command.Item>
  );
}
