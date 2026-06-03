/**
 * EditorPage — the document editor (Phase 7.1).
 *
 * Closes the document lifecycle: the Builder *generates* a program (a tree of
 * blocks); the Editor lets a volunteer *refine* it manually — fix a typo in a
 * liturgy line, add an announcement, drop a block that doesn't belong — and
 * re-render the PDF.
 *
 * Layout mirrors BuilderPage: a left panel (project + document picker), a
 * center column (the block tree editor), and a right preview pane. The
 * render→compile chain is the exact same pair the Builder runs
 * (`bulletin.render` → `bulletin.typstCompile`), reused here.
 *
 * Block CRUD goes through `ipc.block.*`; each successful mutation invalidates
 * the blocks query so the list refetches. A document must be selected before
 * blocks load (a block belongs to one).
 */

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertCircle, FilePlus2, Loader2, Sparkles, Wand2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { Block } from "@/lib/bindings";
import { PdfPreview } from "@/features/builder/PdfPreview";
import { DocumentSelector } from "./DocumentSelector";
import { BlockList } from "./BlockList";

/** Query key for a document's blocks — keep in one place so mutations can invalidate. */
const blocksKey = (documentId: string) => ["blocks", documentId] as const;

/** Pull a readable message out of whatever a mutation rejected with. */
function errMessage(err: unknown, fallback: string): string {
  if (err instanceof IPCError) return `${err.code} — ${err.message}`;
  if (err instanceof Error) return err.message;
  return fallback;
}

export function EditorPage() {
  const qc = useQueryClient();
  const [projectId, setProjectId] = useState("");
  const [documentId, setDocumentId] = useState("");
  const [pdfBase64, setPdfBase64] = useState<string | null>(null);

  // ── Blocks for the chosen document ──────────────────────────────────────────
  const blocks = useQuery({
    queryKey: blocksKey(documentId),
    queryFn: () => ipc.block.list(documentId),
    enabled: !!documentId,
  });

  const invalidateBlocks = () =>
    qc.invalidateQueries({ queryKey: blocksKey(documentId) });

  // ── Block CRUD ───────────────────────────────────────────────────────────────
  const addBlock = useMutation({
    // New blocks are top-level (`parent_id: null`) with an empty JSON payload;
    // the user fills in the data in the card.
    mutationFn: () => ipc.block.create(documentId, null, "text", "{}"),
    onSuccess: invalidateBlocks,
  });

  const updateBlock = useMutation({
    mutationFn: (v: { id: string; kind: string; data: string }) =>
      ipc.block.update(v.id, v.kind, v.data),
    onSuccess: invalidateBlocks,
  });

  const deleteBlock = useMutation({
    // `block_delete` cascades to the subtree on the backend.
    mutationFn: (id: string) => ipc.block.delete(id),
    onSuccess: invalidateBlocks,
  });

  // ── Render → compile (same chain as the Builder) ─────────────────────────────
  const renderAndCompile = useMutation({
    mutationFn: async (docId: string) => {
      const source = await ipc.bulletin.render(docId);
      return ipc.bulletin.typstCompile(source);
    },
    onSuccess: (base64) => setPdfBase64(base64),
  });

  const busy =
    addBlock.isPending || updateBlock.isPending || deleteBlock.isPending;

  const onSelectProject = (id: string) => {
    setProjectId(id);
    setDocumentId(""); // a different project's documents differ
    setPdfBase64(null);
  };
  const onSelectDocument = (id: string) => {
    setDocumentId(id);
    setPdfBase64(null);
  };

  const blockList: Block[] = blocks.data ?? [];
  const mutationError =
    addBlock.error ?? updateBlock.error ?? deleteBlock.error ?? null;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Left: pickers */}
      <div className="flex w-[340px] shrink-0 flex-col overflow-hidden border-r border-[var(--color-border)]">
        <header className="border-b border-[var(--color-border)] px-6 py-4">
          <div className="text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 7.1 · Editor
          </div>
          <h1 className="mt-0.5 text-[var(--text-ui-xl)] font-bold">
            Dokumenteditor
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Finjuster blokkene i et eksisterende dokument.
          </p>
        </header>

        <div className="flex-1 overflow-y-auto p-6">
          <DocumentSelector
            projectId={projectId}
            documentId={documentId}
            onSelectProject={onSelectProject}
            onSelectDocument={onSelectDocument}
          />
        </div>
      </div>

      {/* Center: block tree */}
      <div className="flex min-w-0 flex-[1.2] flex-col overflow-hidden border-r border-[var(--color-border)] p-6">
        {!documentId ? (
          <div className="grid h-full place-items-center">
            <p className="max-w-xs text-center text-sm text-[var(--color-fg-muted)]">
              Velg et dokument til venstre for å redigere blokkene.
            </p>
          </div>
        ) : blocks.isPending ? (
          <div className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
            <Loader2 size={14} className="animate-spin" />
            Laster blokker…
          </div>
        ) : blocks.isError ? (
          <ErrorBanner
            message={errMessage(blocks.error, "Kunne ikke laste blokker")}
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
            <BlockList
              blocks={blockList}
              busy={busy}
              onAdd={() => addBlock.mutate()}
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
            <PdfPreview base64={pdfBase64} fileName="dokument.pdf" />
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
                    ? "Trykk «Forhåndsvis PDF» for å se dokumentet."
                    : "Velg et dokument for å forhåndsvise det."}
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
