/**
 * Typst template builder (Phase 4.2).
 *
 * Templates are stored in the `template` table (id/name/kind/source) and bound
 * to documents, but until now there was no UI to author the Typst `source` —
 * only a raw data layer. This panel closes that gap so a volunteer can craft a
 * layout without touching code.
 *
 * Three columns:
 *   - LEFT  — templates grouped by kind (program / song_sheet / poster / form),
 *             with a "new template" action.
 *   - CENTER— a Typst source editor with lightweight syntax highlighting (an
 *             overlay behind the textarea), live `{{ var }}` hints, and an
 *             inline lint that catches the cheap mistakes (`validateTypst`).
 *   - RIGHT — a live preview: sample values are injected into the source
 *             (`injectSampleData`) and the result is compiled to PDF through the
 *             same `bulletin.typstCompile` chain the Builder/Editor use.
 *
 * Data flows entirely through `ipc.template.*` (CRUD) and `ipc.bulletin`
 * (render preview). No backend changes were needed — the repo + commands have
 * shipped since Phase 1.1; this is the missing front door.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  FileText,
  Music,
  Image as ImageIcon,
  ClipboardList,
  Plus,
  Trash2,
  Save,
  Loader2,
  Eye,
  AlertTriangle,
  Braces,
  FolderOpen,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { Template } from "@/lib/bindings";
import { cn } from "@/lib/cn";

import {
  extractVariables,
  injectSampleData,
  validateTypst,
  type LintIssue,
} from "./typst-lint";

// ── Kinds ─────────────────────────────────────────────────────────────────────

const KINDS = ["program", "song_sheet", "poster", "form"] as const;
type Kind = (typeof KINDS)[number];

const KIND_LABELS: Record<Kind, string> = {
  program: "Program",
  song_sheet: "Sangark",
  poster: "Plakat",
  form: "Skjema",
};

function KindIcon({ kind, size = 16 }: { kind: string; size?: number }) {
  const props = { size, "aria-hidden": true } as const;
  switch (kind) {
    case "song_sheet":
      return <Music {...props} />;
    case "poster":
      return <ImageIcon {...props} />;
    case "form":
      return <ClipboardList {...props} />;
    default:
      return <FileText {...props} />;
  }
}

// A starter skeleton so a brand-new template isn't a blank page.
function starterSource(kind: Kind): string {
  return [
    '#set page(paper: "a4", margin: 2cm)',
    '#set text(font: "Inter", size: 11pt)',
    "",
    `#align(center)[#text(size: 20pt, weight: "bold")[{{ title }}]]`,
    "",
    `#align(center)[{{ church_name }} — {{ date }}]`,
    "",
    "#line(length: 100%)",
    "",
    kind === "song_sheet"
      ? "{{ song_body }}"
      : kind === "poster"
        ? `#v(2cm)\n#align(center)[#text(size: 16pt)[{{ subtitle }}]]`
        : "{{ body }}",
    "",
  ].join("\n");
}

// Sample values for the live preview, keyed by placeholder name. Anything not
// listed falls back to `[name]` so the author can spot an unfilled slot.
const SAMPLE_DATA: Record<string, string> = {
  title: "Høymesse",
  subtitle: "Velkommen til gudstjeneste",
  church_name: "Sankt Hallvard menighet",
  date: "Søndag 7. juni 2026",
  body: "Inngangssalme, syndsbekjennelse, dagens tekst og forbønn.",
  song_body: "1. Navn over alle navn,\n   du dyrebare ord …",
};

// ── Error formatting ──────────────────────────────────────────────────────────

function fmtError(err: unknown): string {
  if (err instanceof IPCError) return `${err.code} — ${err.message}`;
  if (err instanceof Error) return err.message;
  return String(err);
}

// ── Syntax highlighter ──────────────────────────────────────────────────────
// Minimal, dependency-free Typst highlighting: we render a colourised <pre>
// behind a transparent <textarea>, so the caret/selection stay native while the
// text appears highlighted. Tokens: #functions, "strings", {{placeholders}},
// // comments. Everything is HTML-escaped first.

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

const HL_PLACEHOLDER = /\{\{\s*[A-Za-z_][A-Za-z0-9_]*\s*\}\}/g;
const HL_STRING = /"[^"\n]*"/g;
const HL_FUNC = /#[A-Za-z_][A-Za-z0-9_.]*/g;
const HL_COMMENT = /\/\/[^\n]*/g;

/** Turn Typst source into highlighted HTML. Pure + order-safe via tokenising. */
function highlightTypst(source: string): string {
  // Tokenise into non-overlapping spans, longest-priority first.
  type Tok = { start: number; end: number; cls: string };
  const toks: Tok[] = [];
  const claim = (re: RegExp, cls: string) => {
    for (const m of source.matchAll(re)) {
      const start = m.index ?? 0;
      const end = start + m[0].length;
      if (toks.some((t) => start < t.end && end > t.start)) continue;
      toks.push({ start, end, cls });
    }
  };
  claim(HL_COMMENT, "tok-comment");
  claim(HL_STRING, "tok-string");
  claim(HL_PLACEHOLDER, "tok-var");
  claim(HL_FUNC, "tok-func");
  toks.sort((a, b) => a.start - b.start);

  let html = "";
  let cursor = 0;
  for (const t of toks) {
    if (t.start < cursor) continue;
    html += escapeHtml(source.slice(cursor, t.start));
    html += `<span class="${t.cls}">${escapeHtml(source.slice(t.start, t.end))}</span>`;
    cursor = t.end;
  }
  html += escapeHtml(source.slice(cursor));
  // A trailing newline must be visible so the overlay height matches the
  // textarea's scroll height.
  return html + "\n";
}

// ── TemplateEditor (center column) ─────────────────────────────────────────

function TemplateEditor({
  source,
  onChange,
}: {
  source: string;
  onChange: (next: string) => void;
}) {
  const preRef = useRef<HTMLPreElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  // Keep the highlight overlay scroll-synced with the textarea.
  const onScroll = () => {
    if (preRef.current && taRef.current) {
      preRef.current.scrollTop = taRef.current.scrollTop;
      preRef.current.scrollLeft = taRef.current.scrollLeft;
    }
  };

  return (
    <div className="relative h-full overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] font-mono text-[13px] leading-[1.5]">
      <pre
        ref={preRef}
        aria-hidden
        className="pointer-events-none absolute inset-0 m-0 overflow-auto whitespace-pre-wrap break-words p-3 text-transparent [&_.tok-comment]:text-[var(--color-fg-muted)] [&_.tok-comment]:italic [&_.tok-func]:text-[var(--color-accent)] [&_.tok-string]:text-[oklch(0.74_0.18_145)] [&_.tok-var]:text-[oklch(0.7_0.16_280)] [&_.tok-var]:font-semibold"
        dangerouslySetInnerHTML={{ __html: highlightTypst(source) }}
      />
      <textarea
        ref={taRef}
        value={source}
        onChange={(e) => onChange(e.target.value)}
        onScroll={onScroll}
        spellCheck={false}
        aria-label="Typst-kilde"
        className="absolute inset-0 h-full w-full resize-none overflow-auto whitespace-pre-wrap break-words bg-transparent p-3 text-[var(--color-fg)] caret-[var(--color-accent)] outline-none"
      />
    </div>
  );
}

// ── Variable hints + lint (under the editor) ─────────────────────────────────

function HintsBar({ source, issues }: { source: string; issues: LintIssue[] }) {
  const vars = useMemo(() => extractVariables(source), [source]);
  const errors = issues.filter((i) => i.severity === "error");

  return (
    <div className="flex flex-col gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-2.5 text-xs">
      <div className="flex items-center gap-2">
        <Braces
          size={13}
          className="text-[var(--color-fg-muted)]"
          aria-hidden
        />
        <span className="text-[var(--color-fg-muted)]">Variabler:</span>
        {vars.length === 0 ? (
          <span className="text-[var(--color-fg-muted)] italic">
            ingen {"{{ … }}"} ennå
          </span>
        ) : (
          <div className="flex flex-wrap gap-1">
            {vars.map((v) => (
              <span
                key={v.name}
                className="rounded-full border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2 py-0.5 font-mono text-[11px] text-[oklch(0.7_0.16_280)]"
              >
                {`{{ ${v.name} }}`}
                {v.count > 1 && (
                  <span className="ml-1 text-[var(--color-fg-muted)]">
                    ×{v.count}
                  </span>
                )}
              </span>
            ))}
          </div>
        )}
      </div>

      {issues.length > 0 && (
        <ul className="flex flex-col gap-1">
          {issues.map((i, idx) => (
            <li
              key={idx}
              className={cn(
                "flex items-center gap-1.5",
                i.severity === "error"
                  ? "text-[var(--color-danger)]"
                  : "text-[oklch(0.7_0.16_75)]",
              )}
            >
              <AlertTriangle size={12} aria-hidden />
              {i.message}
            </li>
          ))}
        </ul>
      )}
      {errors.length === 0 && issues.length === 0 && source.trim() !== "" && (
        <span className="text-[oklch(0.6_0.14_145)]">
          Ingen åpenbare syntaksfeil.
        </span>
      )}
    </div>
  );
}

// ── New-template form ─────────────────────────────────────────────────────────

function NewTemplateForm({
  onCreate,
  onCancel,
  isPending,
}: {
  onCreate: (name: string, kind: Kind) => void;
  onCancel: () => void;
  isPending: boolean;
}) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<Kind>("program");

  return (
    <form
      className="flex flex-col gap-2.5 rounded-lg border border-[var(--color-accent)] bg-[var(--color-bg-elevated)] p-3"
      onSubmit={(e) => {
        e.preventDefault();
        const trimmed = name.trim();
        if (trimmed) onCreate(trimmed, kind);
      }}
    >
      <input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Navn på mal …"
        aria-label="Navn på mal"
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm outline-none focus:border-[var(--color-accent)]"
      />
      <div className="flex flex-wrap gap-1.5">
        {KINDS.map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => setKind(k)}
            className={cn(
              "flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium transition-colors",
              kind === k
                ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_15%,transparent)] text-[var(--color-accent)]"
                : "border-[var(--color-border)] text-[var(--color-fg-muted)] hover:border-[var(--color-fg-muted)]",
            )}
          >
            <KindIcon kind={k} size={11} />
            {KIND_LABELS[k]}
          </button>
        ))}
      </div>
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!name.trim() || isPending}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-[var(--color-accent)] py-1.5 text-xs font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
        >
          {isPending ? <Loader2 size={12} className="animate-spin" /> : null}
          Opprett
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
        >
          Avbryt
        </button>
      </div>
    </form>
  );
}

// ── Live preview (right column) ───────────────────────────────────────────────

function LivePreview({ source }: { source: string }) {
  const [base64, setBase64] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const compile = useMutation({
    mutationFn: (src: string) => {
      const filled = injectSampleData(src, SAMPLE_DATA);
      return ipc.bulletin.typstCompile(filled);
    },
    onSuccess: (b64) => {
      setBase64(b64);
      setError(null);
    },
    onError: (err) => setError(fmtError(err)),
  });

  // Debounced re-compile whenever the (lint-clean) source changes.
  const issues = useMemo(() => validateTypst(source), [source]);
  const hasError = issues.some((i) => i.severity === "error");

  useEffect(() => {
    if (hasError || source.trim() === "") return;
    const id = setTimeout(() => compile.mutate(source), 500);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, hasError]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2.5">
        <Eye size={14} className="text-[var(--color-accent)]" aria-hidden />
        <span className="text-sm font-semibold">Forhåndsvisning</span>
        {compile.isPending && (
          <Loader2
            size={13}
            className="animate-spin text-[var(--color-fg-muted)]"
          />
        )}
        <span className="ml-auto text-[11px] text-[var(--color-fg-muted)]">
          eksempeldata
        </span>
      </div>
      <div className="flex-1 overflow-auto p-4">
        {hasError ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center text-sm text-[var(--color-danger)]">
            <AlertTriangle size={28} aria-hidden />
            Rett opp syntaksfeilene for å se forhåndsvisning.
          </div>
        ) : error ? (
          <div
            role="alert"
            className="rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-4 py-3 text-sm text-[var(--color-danger)]"
          >
            Kunne ikke kompilere: {error}
          </div>
        ) : base64 ? (
          <embed
            title="Forhåndsvisning"
            src={`data:application/pdf;base64,${base64}`}
            type="application/pdf"
            className="h-full min-h-[400px] w-full rounded-md border border-[var(--color-border)]"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-[var(--color-fg-muted)]">
            Skriv i editoren for å bygge forhåndsvisning.
          </div>
        )}
      </div>
    </div>
  );
}

// ── TemplatesPanel ─────────────────────────────────────────────────────────────

const QUERY_KEY = ["templates"] as const;

export function TemplatesPanel() {
  const qc = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draftSource, setDraftSource] = useState("");
  const [draftName, setDraftName] = useState("");
  const [creating, setCreating] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const query = useQuery({
    queryKey: QUERY_KEY,
    queryFn: () => ipc.template.list(),
  });
  const invalidate = () => qc.invalidateQueries({ queryKey: QUERY_KEY });

  const templates: Template[] = useMemo(() => query.data ?? [], [query.data]);
  const selected = useMemo(
    () => templates.find((t) => t.id === selectedId) ?? null,
    [templates, selectedId],
  );

  // Load the selected template's source/name into the editable draft.
  useEffect(() => {
    if (selected) {
      setDraftSource(selected.source);
      setDraftName(selected.name);
    }
  }, [selected]);

  const grouped = useMemo(() => {
    const map = new Map<string, Template[]>();
    for (const t of templates) {
      const list = map.get(t.kind) ?? [];
      list.push(t);
      map.set(t.kind, list);
    }
    return map;
  }, [templates]);

  const issues = useMemo(() => validateTypst(draftSource), [draftSource]);

  // ── Mutations ──────────────────────────────────────────────────────────────

  const createMutation = useMutation({
    mutationFn: (v: { name: string; kind: Kind }) =>
      ipc.template.create(v.name, v.kind, starterSource(v.kind)),
    onSuccess: (t) => {
      setCreating(false);
      setSelectedId(t.id);
      setErrorMsg(null);
      invalidate();
    },
    onError: (err) => setErrorMsg(fmtError(err)),
  });

  const saveMutation = useMutation({
    mutationFn: (v: {
      id: string;
      name: string;
      kind: string;
      source: string;
    }) => ipc.template.update(v.id, v.name, v.kind, v.source),
    onSuccess: () => {
      setErrorMsg(null);
      invalidate();
    },
    onError: (err) => setErrorMsg(fmtError(err)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => ipc.template.delete(id),
    onSuccess: () => {
      setSelectedId(null);
      setErrorMsg(null);
      invalidate();
    },
    onError: (err) => setErrorMsg(fmtError(err)),
  });

  const dirty =
    selected != null &&
    (selected.source !== draftSource || selected.name !== draftName);

  return (
    <div className="flex h-full overflow-hidden">
      {/* LEFT — template list, grouped by kind */}
      <aside className="flex w-[260px] shrink-0 flex-col overflow-hidden border-r border-[var(--color-border)]">
        <header className="border-b border-[var(--color-border)] px-4 py-4">
          <div className="text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 4.2 · Maler
          </div>
          <h1 className="mt-0.5 text-[var(--text-ui-xl)] font-bold">
            Malbygger
          </h1>
        </header>

        <div className="flex-1 overflow-y-auto p-3">
          {creating ? (
            <NewTemplateForm
              isPending={createMutation.isPending}
              onCancel={() => setCreating(false)}
              onCreate={(name, kind) => createMutation.mutate({ name, kind })}
            />
          ) : (
            <button
              type="button"
              onClick={() => setCreating(true)}
              className="mb-3 flex w-full items-center justify-center gap-2 rounded-md border border-[var(--color-border)] px-2.5 py-2 text-sm font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)]"
            >
              <Plus size={14} />
              Ny mal
            </button>
          )}

          {query.isError ? (
            <p className="px-1 py-4 text-sm text-[var(--color-danger)]">
              Kunne ikke laste maler: {fmtError(query.error)}
            </p>
          ) : templates.length === 0 && !query.isPending ? (
            <div className="flex flex-col items-center gap-2 py-12 text-center">
              <FolderOpen
                size={32}
                className="text-[var(--color-fg-muted)] opacity-40"
              />
              <p className="text-sm text-[var(--color-fg-muted)]">
                Ingen maler ennå. Lag den første.
              </p>
            </div>
          ) : (
            KINDS.filter((k) => (grouped.get(k)?.length ?? 0) > 0).map(
              (kind) => (
                <div key={kind} className="mb-3">
                  <div className="mb-1 flex items-center gap-1.5 px-1 text-[11px] font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
                    <KindIcon kind={kind} size={12} />
                    {KIND_LABELS[kind]}
                  </div>
                  <ul className="space-y-0.5">
                    {(grouped.get(kind) ?? []).map((t) => (
                      <li key={t.id}>
                        <button
                          type="button"
                          onClick={() => setSelectedId(t.id)}
                          className={cn(
                            "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-sm transition-colors",
                            selectedId === t.id
                              ? "bg-[var(--color-bg-surface)] font-medium text-[var(--color-fg)]"
                              : "text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]/60 hover:text-[var(--color-fg)]",
                          )}
                        >
                          <span className="truncate">{t.name}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              ),
            )
          )}
        </div>
      </aside>

      {/* CENTER + RIGHT */}
      {selected ? (
        <div className="flex flex-1 overflow-hidden">
          {/* CENTER — editor */}
          <section className="flex min-w-0 flex-1 flex-col overflow-hidden border-r border-[var(--color-border)]">
            <header className="flex items-center gap-3 border-b border-[var(--color-border)] px-4 py-2.5">
              <input
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                aria-label="Malnavn"
                className="min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 text-sm font-semibold outline-none hover:border-[var(--color-border)] focus:border-[var(--color-accent)]"
              />
              <button
                type="button"
                aria-label="Lagre mal"
                disabled={
                  !dirty || draftName.trim() === "" || saveMutation.isPending
                }
                onClick={() =>
                  saveMutation.mutate({
                    id: selected.id,
                    name: draftName.trim(),
                    kind: selected.kind,
                    source: draftSource,
                  })
                }
                className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-xs font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-40"
              >
                {saveMutation.isPending ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <Save size={12} />
                )}
                Lagre
              </button>
              <button
                type="button"
                aria-label={`Slett ${selected.name}`}
                onClick={() => deleteMutation.mutate(selected.id)}
                className="rounded-md p-1.5 text-[var(--color-fg-muted)] transition-colors hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-danger)]"
              >
                <Trash2 size={14} />
              </button>
            </header>

            {errorMsg && (
              <div
                role="alert"
                className="border-b border-[var(--color-border)] bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-4 py-2 text-xs text-[var(--color-danger)]"
              >
                {errorMsg}
              </div>
            )}

            <div className="min-h-0 flex-1 p-3">
              <TemplateEditor source={draftSource} onChange={setDraftSource} />
            </div>
            <HintsBar source={draftSource} issues={issues} />
          </section>

          {/* RIGHT — live preview */}
          <section className="flex w-[44%] min-w-[320px] shrink-0 flex-col overflow-hidden bg-[var(--color-bg-elevated)]">
            <LivePreview source={draftSource} />
          </section>
        </div>
      ) : (
        <div className="grid flex-1 place-items-center">
          <div className="max-w-sm text-center">
            <LayoutTemplateHint />
            <p className="mt-3 text-sm text-[var(--color-fg-muted)]">
              Velg en mal til venstre, eller lag en ny for å begynne å redigere
              Typst-kilden med live forhåndsvisning.
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

function LayoutTemplateHint() {
  return (
    <FileText
      size={44}
      className="mx-auto text-[var(--color-fg-muted)] opacity-30"
      aria-hidden
    />
  );
}
