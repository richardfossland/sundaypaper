/**
 * FormsPage — the FormBuilder (Phase 7.2).
 *
 * Churches need fillable paper/PDF forms: signup sheets, attendance forms,
 * donation cards, consent slips. A form is just a document (`kind: "form"`)
 * whose block tree is made of the form-field block kinds the layout engine
 * renders — `form_field` (a labelled blank), `checkbox` (a tick box) and
 * `signature` (a sign-on rule). The page mirrors the Editor: a left panel
 * (project picker + a "new form" action), a center FormBuilder (the block tree
 * with a quick-add palette for the three field kinds) and a right preview pane
 * driven by the very same `bulletin.render → bulletin.typstCompile` chain the
 * Builder and Editor use.
 *
 * Privacy is the headline: SundayPaper renders forms as *printed* fields a
 * person fills in by hand, so member/form data never has to leave the machine
 * to produce the document. The banner makes that promise visible (CLAUDE.md).
 */

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckSquare,
  FilePlus2,
  Loader2,
  PenLine,
  Plus,
  ShieldCheck,
  Sparkles,
  TextCursorInput,
  Wand2,
} from "lucide-react";

import { ipc, errMessage } from "@/lib/ipc";
import type { Block } from "@/lib/bindings";
import { PdfPreview } from "@/features/builder/PdfPreview";
import { DocumentSelector } from "@/features/editor/DocumentSelector";
import { BlockList } from "@/features/editor/BlockList";
import { documentsKey } from "@/lib/queryKeys";

/** Query key for a document's blocks — kept local so mutations can invalidate. */
const blocksKey = (documentId: string) => ["blocks", documentId] as const;

/**
 * The three quick-add field kinds and the JSON skeleton each new block gets.
 * Pre-seeding the payload means a volunteer sees a usable field immediately and
 * only has to fill in the label, rather than authoring JSON from scratch.
 */
const FIELD_PALETTE = [
  {
    kind: "form_field",
    label: "Tekstfelt",
    icon: TextCursorInput,
    data: JSON.stringify({ label: "Navn", hint: null, width: "full" }),
  },
  {
    kind: "checkbox",
    label: "Avkrysning",
    icon: CheckSquare,
    data: JSON.stringify({ label: "Jeg samtykker" }),
  },
  {
    kind: "signature",
    label: "Signatur",
    icon: PenLine,
    data: JSON.stringify({ label: "Signatur og dato", width: "half" }),
  },
] as const;

export function FormsPage() {
  const qc = useQueryClient();
  const [projectId, setProjectId] = useState("");
  const [documentId, setDocumentId] = useState("");
  const [pdfBase64, setPdfBase64] = useState<string | null>(null);

  // ── Blocks for the chosen form ───────────────────────────────────────────────
  const blocks = useQuery({
    queryKey: blocksKey(documentId),
    queryFn: () => ipc.block.list(documentId),
    enabled: !!documentId,
  });

  const invalidateBlocks = () =>
    qc.invalidateQueries({ queryKey: blocksKey(documentId) });

  // ── Create a new form document, then select it ───────────────────────────────
  const createForm = useMutation({
    mutationFn: () =>
      ipc.document.create(projectId, "Nytt skjema", "form", "A4"),
    onSuccess: (doc) => {
      qc.invalidateQueries({ queryKey: documentsKey(projectId) });
      setDocumentId(doc.id);
      setPdfBase64(null);
    },
  });

  // ── Field CRUD ────────────────────────────────────────────────────────────────
  const addField = useMutation({
    // New fields are top-level (`parent_id: null`); the palette supplies the
    // kind + a pre-filled JSON skeleton so the field is usable right away.
    mutationFn: (v: { kind: string; data: string }) =>
      ipc.block.create(documentId, null, v.kind, v.data),
    onSuccess: invalidateBlocks,
  });

  const updateBlock = useMutation({
    mutationFn: (v: { id: string; kind: string; data: string }) =>
      ipc.block.update(v.id, v.kind, v.data),
    onSuccess: invalidateBlocks,
  });

  const deleteBlock = useMutation({
    mutationFn: (id: string) => ipc.block.delete(id),
    onSuccess: invalidateBlocks,
  });

  // ── Render → compile (same chain as Builder/Editor) ──────────────────────────
  const renderAndCompile = useMutation({
    mutationFn: async (docId: string) => {
      const source = await ipc.bulletin.render(docId);
      return ipc.bulletin.typstCompile(source);
    },
    onSuccess: (base64) => setPdfBase64(base64),
  });

  const busy =
    addField.isPending || updateBlock.isPending || deleteBlock.isPending;

  const onSelectProject = (id: string) => {
    setProjectId(id);
    setDocumentId("");
    setPdfBase64(null);
  };
  const onSelectDocument = (id: string) => {
    setDocumentId(id);
    setPdfBase64(null);
  };

  const blockList: Block[] = blocks.data ?? [];
  const mutationError =
    createForm.error ??
    addField.error ??
    updateBlock.error ??
    deleteBlock.error ??
    null;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Left: project picker + new-form action */}
      <div className="flex w-[340px] shrink-0 flex-col overflow-hidden border-r border-[var(--color-border)]">
        <header className="border-b border-[var(--color-border)] px-6 py-4">
          <div className="text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 7.2 · Skjema
          </div>
          <h1 className="mt-0.5 text-[var(--text-ui-xl)] font-bold">
            Skjemabygger
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Lag utfyllbare skjema — påmelding, oppmøte, gaver.
          </p>
        </header>

        <div className="flex-1 overflow-y-auto p-6">
          <DocumentSelector
            projectId={projectId}
            documentId={documentId}
            onSelectProject={onSelectProject}
            onSelectDocument={onSelectDocument}
          />

          {projectId && (
            <button
              type="button"
              onClick={() => createForm.mutate()}
              disabled={createForm.isPending}
              className="mt-4 flex w-full items-center justify-center gap-2 rounded-md border border-[var(--color-border)] px-2.5 py-2 text-sm font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
            >
              {createForm.isPending ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <FilePlus2 size={14} />
              )}
              Nytt skjema
            </button>
          )}

          {/* The privacy promise, made visible (CLAUDE.md). */}
          <div className="mt-6 flex items-start gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3 text-xs text-[var(--color-fg-muted)]">
            <ShieldCheck
              size={16}
              className="mt-0.5 shrink-0 text-[var(--color-accent)]"
            />
            <span>
              Skjemafelt skrives ut som blanke linjer og fylles ut for hånd.
              Person- og skjemadata forlater aldri maskinen.
            </span>
          </div>
        </div>
      </div>

      {/* Center: the field tree + quick-add palette */}
      <div className="flex min-w-0 flex-[1.2] flex-col overflow-hidden border-r border-[var(--color-border)] p-6">
        {!documentId ? (
          <div className="grid h-full place-items-center">
            <p className="max-w-xs text-center text-sm text-[var(--color-fg-muted)]">
              Velg eller lag et skjema til venstre for å legge til felt.
            </p>
          </div>
        ) : blocks.isPending ? (
          <div className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
            <Loader2 size={14} className="animate-spin" />
            Laster felt…
          </div>
        ) : blocks.isError ? (
          <ErrorBanner
            message={errMessage(blocks.error, "Kunne ikke laste felt")}
          />
        ) : (
          <>
            {mutationError && (
              <div className="pb-3">
                <ErrorBanner
                  message={errMessage(mutationError, "Handlingen feilet")}
                />
              </div>
            )}

            {/* Quick-add palette for the three field kinds. */}
            <div className="flex flex-wrap gap-2 pb-4">
              {FIELD_PALETTE.map((f) => (
                <button
                  key={f.kind}
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    addField.mutate({ kind: f.kind, data: f.data })
                  }
                  className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
                >
                  <f.icon size={13} />
                  <Plus size={11} className="-mr-0.5" />
                  {f.label}
                </button>
              ))}
            </div>

            <BlockList
              blocks={blockList}
              busy={busy}
              // The "Legg til blokk" button defaults to a text field — the most
              // common form element; the palette above adds the other kinds.
              onAdd={() =>
                addField.mutate({
                  kind: "form_field",
                  data: JSON.stringify({
                    label: "",
                    hint: null,
                    width: "full",
                  }),
                })
              }
              onUpdate={(id, kind, data) =>
                updateBlock.mutate({ id, kind, data })
              }
              onDelete={(id) => deleteBlock.mutate(id)}
            />
          </>
        )}
      </div>

      {/* Right: preview */}
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden p-6">
        {documentId && (
          <div className="pb-3">
            {renderAndCompile.isError && (
              <div className="pb-2">
                <ErrorBanner
                  message={errMessage(
                    renderAndCompile.error,
                    "Kunne ikke kompilere PDF",
                  )}
                />
              </div>
            )}
            <button
              type="button"
              onClick={() => renderAndCompile.mutate(documentId)}
              disabled={renderAndCompile.isPending}
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-[var(--color-accent)] px-3 py-2.5 text-sm font-bold text-[var(--color-accent-fg)] shadow-sm transition-all hover:brightness-110 active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50"
            >
              {renderAndCompile.isPending ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Wand2 size={16} />
              )}
              Forhåndsvis PDF
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1">
          {pdfBase64 ? (
            <PdfPreview base64={pdfBase64} fileName="skjema.pdf" />
          ) : (
            <div className="grid h-full place-items-center rounded-xl border border-dashed border-[var(--color-border)]">
              <div className="max-w-xs text-center">
                {documentId ? (
                  <Sparkles
                    size={40}
                    className="mx-auto mb-3 text-[var(--color-fg-muted)] opacity-40"
                  />
                ) : (
                  <FilePlus2
                    size={40}
                    className="mx-auto mb-3 text-[var(--color-fg-muted)] opacity-40"
                  />
                )}
                <p className="text-sm text-[var(--color-fg-muted)]">
                  {documentId
                    ? "Trykk «Forhåndsvis PDF» for å se skjemaet."
                    : "Velg et skjema for å forhåndsvise det."}
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── ErrorBanner ───────────────────────────────────────────────────────────────

function ErrorBanner({ message }: { message: string }) {
  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-lg bg-[color-mix(in_oklch,var(--color-danger)_10%,transparent)] px-3 py-2 text-sm text-[var(--color-danger)]"
    >
      <AlertCircle size={14} className="mt-0.5 shrink-0" />
      <span>{message}</span>
    </div>
  );
}
