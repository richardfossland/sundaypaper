/**
 * BuilderPage — the FORWARD pipeline made concrete (Phase 4.3).
 *
 * Proves core promise #1: "from a service plan to a finished, printable program
 * in one click". A three-step workflow drives the three already-tested backend
 * commands in sequence:
 *
 *   1. ServicePlanForm — assemble (or sample-seed) a `ServicePlan`.
 *   2. generate  — `bulletin_generate(project, plan)` → a `program` Document
 *                  (a tree of blocks persisted in the project).
 *   3. render + compile — `bulletin_render(doc)` → Typst source, then
 *                  `typst_compile(source)` → base64 PDF, shown inline + downloadable.
 *
 * Each async hop is a TanStack Query mutation so loading / error UI is uniform.
 * The render→compile pair runs as one mutation so the user gets a single
 * "Lag program" button rather than two coupled clicks.
 *
 * A project is required (the generated document belongs to one). We list
 * projects via `ipc.project` and let the user pick or create one inline.
 */

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  FilePlus2,
  FileText,
  FolderPlus,
  Loader2,
  Sparkles,
  Wand2,
} from "lucide-react";

import { ipc, errMessage } from "@/lib/ipc";
import type { Document, ServicePlan } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { projectsKey } from "@/lib/queryKeys";
import { ServicePlanForm } from "./ServicePlanForm";
import { emptyPlan } from "./plan-defaults";
import { PdfPreview } from "./PdfPreview";

export function BuilderPage() {
  const qc = useQueryClient();
  const [plan, setPlan] = useState<ServicePlan>(emptyPlan);
  const [projectId, setProjectId] = useState<string>("");
  const [newProjectName, setNewProjectName] = useState("");
  const [doc, setDoc] = useState<Document | null>(null);
  const [pdfBase64, setPdfBase64] = useState<string | null>(null);

  // ── Projects ──────────────────────────────────────────────────────────────
  const projects = useQuery({
    queryKey: projectsKey,
    queryFn: () => ipc.project.list(),
  });

  const createProject = useMutation({
    mutationFn: (name: string) => ipc.project.create(name),
    onSuccess: (p) => {
      setProjectId(p.id);
      setNewProjectName("");
      qc.invalidateQueries({ queryKey: projectsKey });
    },
  });

  // ── Step 2: generate the program document ───────────────────────────────────
  const generate = useMutation({
    mutationFn: () => {
      if (!projectId) throw new Error("Velg et prosjekt først.");
      if (plan.items.length === 0)
        throw new Error("Planen har ingen poster å bygge fra.");
      return ipc.bulletin.generate(projectId, plan, plan.title ?? undefined);
    },
    onSuccess: (d) => {
      setDoc(d);
      setPdfBase64(null);
    },
  });

  // ── Step 3: render to Typst, then compile to PDF (one chained hop) ───────────
  const renderAndCompile = useMutation({
    mutationFn: async (documentId: string) => {
      const source = await ipc.bulletin.render(documentId);
      return ipc.bulletin.typstCompile(source);
    },
    onSuccess: (base64) => setPdfBase64(base64),
  });

  const canGenerate =
    !!projectId && plan.items.length > 0 && !generate.isPending;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Left: workflow */}
      <div className="flex w-[440px] shrink-0 flex-col overflow-hidden border-r border-[var(--color-border)]">
        <header className="border-b border-[var(--color-border)] px-6 py-4">
          <div className="text-xs font-medium uppercase tracking-widest text-[var(--color-accent)]">
            Phase 4.3 · Bygger
          </div>
          <h1 className="mt-0.5 text-[var(--text-ui-xl)] font-bold">
            Dokumentbygger
          </h1>
          <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
            Fra gudstjenesteplan til ferdig, utskriftsklart program.
          </p>
        </header>

        <div className="flex-1 space-y-6 overflow-y-auto p-6">
          {/* Project picker */}
          <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              1 · Prosjekt
            </h2>
            {projects.isPending ? (
              <div className="flex items-center gap-2 text-sm text-[var(--color-fg-muted)]">
                <Loader2 size={14} className="animate-spin" />
                Laster prosjekter…
              </div>
            ) : projects.isError ? (
              <p className="text-sm text-[var(--color-danger)]">
                Kunne ikke laste prosjekter:{" "}
                {errMessage(projects.error, "ukjent feil")}
              </p>
            ) : (projects.data?.length ?? 0) > 0 ? (
              <select
                aria-label="Velg prosjekt"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm"
              >
                <option value="">Velg prosjekt…</option>
                {projects.data!.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            ) : (
              <p className="text-sm text-[var(--color-fg-muted)]">
                Ingen prosjekter ennå — opprett ett under.
              </p>
            )}

            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                const n = newProjectName.trim();
                if (n) createProject.mutate(n);
              }}
            >
              <input
                aria-label="Nytt prosjektnavn"
                value={newProjectName}
                placeholder="Nytt prosjekt…"
                onChange={(e) => setNewProjectName(e.target.value)}
                className="flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm"
              />
              <button
                type="submit"
                disabled={!newProjectName.trim() || createProject.isPending}
                className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
              >
                {createProject.isPending ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <FolderPlus size={12} />
                )}
                Opprett
              </button>
            </form>
          </section>

          {/* Plan form */}
          <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              2 · Plan
            </h2>
            <ServicePlanForm
              plan={plan}
              onChange={setPlan}
              disabled={generate.isPending || renderAndCompile.isPending}
            />
          </section>

          {/* Generate */}
          <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
              3 · Bygg program
            </h2>

            {generate.isError && (
              <ErrorBanner
                message={errMessage(
                  generate.error,
                  "Kunne ikke bygge dokument",
                )}
              />
            )}

            <button
              type="button"
              onClick={() => generate.mutate()}
              disabled={!canGenerate}
              className={cn(
                "flex w-full items-center justify-center gap-2 rounded-lg px-3 py-2.5 text-sm font-bold transition-all",
                "bg-[var(--color-accent)] text-[var(--color-accent-fg)] shadow-sm hover:brightness-110 active:translate-y-px",
                "disabled:cursor-not-allowed disabled:opacity-50",
              )}
            >
              {generate.isPending ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <FilePlus2 size={16} />
              )}
              Generer dokument
            </button>

            {doc && (
              <div className="space-y-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3">
                <div className="flex items-center gap-2 text-sm">
                  <FileText size={14} className="text-[var(--color-accent)]" />
                  <span className="truncate font-medium">{doc.title}</span>
                  <span className="text-xs text-[var(--color-fg-muted)]">
                    {doc.kind}
                  </span>
                </div>

                {renderAndCompile.isError && (
                  <ErrorBanner
                    message={errMessage(
                      renderAndCompile.error,
                      "Kunne ikke kompilere PDF",
                    )}
                  />
                )}

                <button
                  type="button"
                  onClick={() => renderAndCompile.mutate(doc.id)}
                  disabled={renderAndCompile.isPending}
                  className="flex w-full items-center justify-center gap-2 rounded-md border border-[var(--color-accent)] px-3 py-2 text-sm font-semibold text-[var(--color-accent)] transition-colors hover:bg-[color-mix(in_oklch,var(--color-accent)_10%,transparent)] disabled:opacity-50"
                >
                  {renderAndCompile.isPending ? (
                    <Loader2 size={15} className="animate-spin" />
                  ) : (
                    <Wand2 size={15} />
                  )}
                  Lag PDF
                </button>
              </div>
            )}
          </section>
        </div>
      </div>

      {/* Right: preview */}
      <div className="min-w-0 flex-1 p-6">
        {pdfBase64 ? (
          <PdfPreview
            base64={pdfBase64}
            fileName={`${plan.title ?? "program"}.pdf`}
          />
        ) : (
          <div className="grid h-full place-items-center rounded-xl border border-dashed border-[var(--color-border)]">
            <div className="max-w-xs text-center">
              <Sparkles
                size={40}
                className="mx-auto mb-3 text-[var(--color-fg-muted)] opacity-40"
              />
              <p className="text-sm text-[var(--color-fg-muted)]">
                {doc
                  ? "Trykk «Lag PDF» for å kompilere programmet."
                  : "Bygg et dokument fra planen for å se forhåndsvisningen her."}
              </p>
            </div>
          </div>
        )}
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
