/**
 * Document-template panel — the first UI on top of the doc-template CRUD IPC
 * (`ipc.docTemplate.*`). A document template is a reusable, parameterised
 * document skeleton: a name, a `kind`, a Typst source with `{{name}}`
 * placeholders, and a typed list of variables (`name`, `label`, `kind`,
 * default, required).
 *
 * This is distinct from the existing TemplatesPanel, which sits on the simpler
 * `template` table (id/name/kind/source, no variables). Doc-templates add the
 * variable spec + built-in seeding the suite needs to generate documents from
 * data.
 *
 * Layout is master/detail (mirrors SongsPanel): the template list on the left,
 * an editor on the right. On first load, if the list is empty, we call
 * `seedBuiltins()` once and refetch so a fresh install is not a blank page.
 *
 * Both create AND update carry the full variable spec: `doc_template_update`
 * replaces the template's variables atomically (delete + reinsert), so the
 * variable editor is fully editable for new and existing templates alike.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Loader2, Plus, Save, Trash2, X } from "lucide-react";

import { ipc, IPCError, errMessage } from "@/lib/ipc";
import { docTemplatesKey } from "@/lib/queryKeys";
import type { DocTemplate } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import {
  KIND_LABELS,
  TEMPLATE_KINDS,
  VAR_KINDS,
  VAR_KIND_LABELS,
  docTemplateToForm,
  emptyDocTemplateForm,
  emptyVarRow,
  formToVariables,
  isDocTemplateFormValid,
  placeholdersInSource,
  starterSource,
  type DocTemplateFormState,
  type TemplateKind,
} from "./docTemplateForm";

/** A new, unsaved template is the literal `"new"`; otherwise an id. */
type Selection = { kind: "new" } | { kind: "template"; id: string } | null;

const inputCls =
  "w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-3 py-2 text-sm outline-none focus:border-[var(--color-accent)]";

function kindLabel(kind: string): string {
  return (KIND_LABELS as Record<string, string>)[kind] ?? kind;
}

export function DocTemplatesPanel() {
  const qc = useQueryClient();
  const [selection, setSelection] = useState<Selection>(null);
  const [form, setForm] = useState<DocTemplateFormState>(emptyDocTemplateForm);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const seededRef = useRef(false);

  const query = useQuery({
    queryKey: docTemplatesKey,
    queryFn: () => ipc.docTemplate.list(),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: docTemplatesKey });

  const templates = useMemo(() => query.data ?? [], [query.data]);

  // Seed the built-ins exactly once, when the very first successful load comes
  // back empty. `seededRef` guards against the refetch re-triggering it.
  const seed = useMutation({
    mutationFn: () => ipc.docTemplate.seedBuiltins(),
    onSuccess: invalidate,
  });
  useEffect(() => {
    if (
      query.isSuccess &&
      templates.length === 0 &&
      !seededRef.current &&
      !seed.isPending
    ) {
      seededRef.current = true;
      seed.mutate();
    }
  }, [query.isSuccess, templates.length, seed]);

  const create = useMutation({
    mutationFn: (f: DocTemplateFormState) =>
      ipc.docTemplate.create(
        f.name.trim(),
        f.kind,
        f.typstSource,
        formToVariables(f),
      ),
    onSuccess: (t) => {
      invalidate();
      setSelection({ kind: "template", id: t.id });
    },
  });

  const update = useMutation({
    mutationFn: ({ id, f }: { id: string; f: DocTemplateFormState }) =>
      ipc.docTemplate.update(
        id,
        f.name.trim(),
        f.kind,
        f.typstSource,
        formToVariables(f),
      ),
    onSuccess: invalidate,
  });

  const remove = useMutation({
    mutationFn: (id: string) => ipc.docTemplate.delete(id),
    onSuccess: (_void, id) => {
      invalidate();
      setConfirmDelete(null);
      if (selection?.kind === "template" && selection.id === id) {
        setSelection(null);
      }
    },
  });

  function openTemplate(t: DocTemplate) {
    setSelection({ kind: "template", id: t.id });
    setForm(docTemplateToForm(t));
    setConfirmDelete(null);
    create.reset();
    update.reset();
  }
  function openNew() {
    setSelection({ kind: "new" });
    setForm({
      ...emptyDocTemplateForm,
      typstSource: starterSource(emptyDocTemplateForm.kind),
    });
    setConfirmDelete(null);
    create.reset();
    update.reset();
  }
  function closeEditor() {
    setSelection(null);
    create.reset();
    update.reset();
  }

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isDocTemplateFormValid(form) || !selection) return;
    if (selection.kind === "new") {
      create.mutate(form);
    } else {
      update.mutate({ id: selection.id, f: form });
    }
  }

  const isNew = selection?.kind === "new";
  const saving = create.isPending || update.isPending;
  const saveError = create.error ?? update.error;

  // Wire variable rows ↔ source so the author sees mismatches.
  const usedPlaceholders = useMemo(
    () => placeholdersInSource(form.typstSource),
    [form.typstSource],
  );
  const declaredNames = useMemo(
    () =>
      new Set(
        form.variables.map((v) => v.name.trim()).filter((n) => n.length > 0),
      ),
    [form.variables],
  );
  const undeclared = usedPlaceholders.filter((p) => !declaredNames.has(p));

  return (
    <div className="flex h-full overflow-hidden">
      {/* ── Master: template list ─────────────────────────────────────────── */}
      <section className="flex w-80 shrink-0 flex-col border-r border-[var(--color-border)]">
        <header className="flex items-center justify-between border-b border-[var(--color-border)] px-5 py-4">
          <div>
            <h1 className="text-[var(--text-ui-xl)] font-bold">
              Dokumentmaler
            </h1>
            <p className="mt-0.5 text-xs text-[var(--color-fg-muted)]">
              Gjenbrukbare dokumentskjeletter med variabler
            </p>
          </div>
          {(query.isPending || seed.isPending) && (
            <Loader2
              size={16}
              className="animate-spin text-[var(--color-fg-muted)]"
            />
          )}
        </header>

        <div className="border-b border-[var(--color-border)] px-5 py-2.5">
          <button
            type="button"
            onClick={openNew}
            className="flex w-full items-center justify-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-2 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110"
          >
            <Plus size={14} />
            Ny mal
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-2 py-2">
          {query.isError ? (
            <p className="px-3 py-2 text-sm text-[var(--color-danger)]">
              Kunne ikke laste dokumentmaler:{" "}
              {errMessage(query.error, "ukjent feil")}
            </p>
          ) : seed.isError ? (
            <p className="px-3 py-2 text-sm text-[var(--color-danger)]">
              Kunne ikke legge inn innebygde maler:{" "}
              {errMessage(seed.error, "ukjent feil")}
            </p>
          ) : templates.length === 0 && !query.isPending && !seed.isPending ? (
            <p className="px-3 py-6 text-center text-sm text-[var(--color-fg-muted)]">
              Ingen maler ennå. Opprett en med «Ny mal».
            </p>
          ) : (
            <ul className="space-y-0.5">
              {templates.map((t) => {
                const active =
                  selection?.kind === "template" && selection.id === t.id;
                return (
                  <li key={t.id}>
                    <button
                      type="button"
                      onClick={() => openTemplate(t)}
                      className={cn(
                        "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-left text-sm transition-colors",
                        active
                          ? "bg-[var(--color-bg-surface)] text-[var(--color-fg)]"
                          : "text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]/60 hover:text-[var(--color-fg)]",
                      )}
                    >
                      <FileText size={14} aria-hidden className="shrink-0" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-medium">
                          {t.name}
                        </span>
                        <span className="block truncate text-xs text-[var(--color-fg-muted)]">
                          {kindLabel(t.kind)}
                          {t.variables.length > 0 &&
                            ` · ${t.variables.length} variabler`}
                        </span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </section>

      {/* ── Detail: editor ───────────────────────────────────────────────── */}
      <section className="flex-1 overflow-y-auto">
        {!selection ? (
          <div className="grid h-full place-items-center">
            <div className="max-w-sm text-center text-[var(--color-fg-muted)]">
              <FileText
                size={32}
                className="mx-auto mb-3 opacity-50"
                aria-hidden
              />
              <p className="text-sm">
                Velg en mal fra listen, eller opprett en ny for å redigere.
              </p>
            </div>
          </div>
        ) : (
          <form onSubmit={onSubmit} className="mx-auto max-w-3xl px-8 py-6">
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-[var(--text-ui-lg)] font-bold">
                {isNew ? "Ny dokumentmal" : "Rediger dokumentmal"}
              </h2>
              <button
                type="button"
                aria-label="Lukk redigering"
                onClick={closeEditor}
                className="rounded-md p-1.5 text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)]"
              >
                <X size={16} />
              </button>
            </div>

            <div className="space-y-4">
              <Field label="Navn" required>
                <input
                  value={form.name}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, name: e.target.value }))
                  }
                  placeholder="Høymesse-program"
                  className={inputCls}
                />
              </Field>

              <Field label="Type">
                <div className="flex flex-wrap gap-1.5">
                  {TEMPLATE_KINDS.map((k) => (
                    <button
                      key={k}
                      type="button"
                      onClick={() =>
                        setForm((f) => {
                          // Swap the starter source only if untouched-for-new.
                          const swap =
                            isNew && f.typstSource === starterSource(f.kind);
                          return {
                            ...f,
                            kind: k,
                            typstSource: swap
                              ? starterSource(k)
                              : f.typstSource,
                          };
                        })
                      }
                      className={cn(
                        "rounded-full border px-3 py-1 text-xs font-medium transition-colors",
                        form.kind === k
                          ? "border-[var(--color-accent)] bg-[color-mix(in_oklch,var(--color-accent)_15%,transparent)] text-[var(--color-accent)]"
                          : "border-[var(--color-border)] text-[var(--color-fg-muted)] hover:border-[var(--color-fg-muted)]",
                      )}
                    >
                      {KIND_LABELS[k as TemplateKind]}
                    </button>
                  ))}
                </div>
              </Field>

              <Field label="Typst-kilde">
                <textarea
                  value={form.typstSource}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, typstSource: e.target.value }))
                  }
                  rows={12}
                  placeholder="#set page(...)\n{{title}}"
                  aria-label="Typst-kilde"
                  className={cn(inputCls, "resize-y font-mono text-[13px]")}
                />
                <p className="mt-1 text-xs text-[var(--color-fg-muted)]">
                  Bruk <code>{"{{navn}}"}</code> for variabler.
                  {undeclared.length > 0 && (
                    <span className="ml-1 text-[oklch(0.7_0.16_75)]">
                      Brukt men ikke deklarert: {undeclared.join(", ")}.
                    </span>
                  )}
                </p>
              </Field>

              <VariableEditor form={form} onChange={setForm} />
            </div>

            {saveError && (
              <p className="mt-4 text-sm text-[var(--color-danger)]">
                {saveError instanceof IPCError
                  ? saveError.message
                  : "Kunne ikke lagre malen"}
              </p>
            )}

            <div className="mt-6 flex items-center gap-3">
              <button
                type="submit"
                disabled={!isDocTemplateFormValid(form) || saving}
                className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-bold text-[var(--color-accent-fg)] hover:brightness-110 disabled:opacity-50"
              >
                {saving ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Save size={14} />
                )}
                Lagre
              </button>

              {selection.kind === "template" &&
                (confirmDelete === selection.id ? (
                  <span className="flex items-center gap-2 text-sm">
                    <span className="text-[var(--color-fg-muted)]">
                      Slette for godt?
                    </span>
                    <button
                      type="button"
                      onClick={() => remove.mutate(selection.id)}
                      disabled={remove.isPending}
                      className="rounded-md bg-[var(--color-danger)] px-3 py-1.5 text-sm font-bold text-white hover:brightness-110 disabled:opacity-50"
                    >
                      Slett
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmDelete(null)}
                      className="rounded-md px-3 py-1.5 text-sm text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
                    >
                      Avbryt
                    </button>
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmDelete(selection.id)}
                    className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-fg-muted)] hover:border-[var(--color-danger)] hover:text-[var(--color-danger)]"
                  >
                    <Trash2 size={14} />
                    Slett
                  </button>
                ))}
            </div>

            {remove.isError && (
              <p className="mt-3 text-sm text-[var(--color-danger)]">
                {errMessage(remove.error, "Kunne ikke slette malen")}
              </p>
            )}
          </form>
        )}
      </section>
    </div>
  );
}

// ── Variable editor ────────────────────────────────────────────────────────

function VariableEditor({
  form,
  onChange,
}: {
  form: DocTemplateFormState;
  onChange: React.Dispatch<React.SetStateAction<DocTemplateFormState>>;
}) {
  function updateRow(
    idx: number,
    patch: Partial<DocTemplateFormState["variables"][number]>,
  ) {
    onChange((f) => ({
      ...f,
      variables: f.variables.map((v, i) =>
        i === idx ? { ...v, ...patch } : v,
      ),
    }));
  }
  function addRow() {
    onChange((f) => ({ ...f, variables: [...f.variables, emptyVarRow()] }));
  }
  function removeRow(idx: number) {
    onChange((f) => ({
      ...f,
      variables: f.variables.filter((_, i) => i !== idx),
    }));
  }

  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <span className="text-xs font-medium text-[var(--color-fg-muted)]">
          Variabler
        </span>
        <button
          type="button"
          onClick={addRow}
          className="flex items-center gap-1 rounded-md border border-[var(--color-border)] px-2 py-1 text-xs text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
        >
          <Plus size={12} />
          Legg til
        </button>
      </div>

      {form.variables.length === 0 ? (
        <p className="rounded-md border border-dashed border-[var(--color-border)] px-3 py-4 text-center text-xs text-[var(--color-fg-muted)]">
          Ingen variabler. Legg til en for å parametrisere malen.
        </p>
      ) : (
        <ul className="space-y-2">
          {form.variables.map((v, idx) => (
            <li
              key={idx}
              className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-2.5"
            >
              <div className="grid grid-cols-2 gap-2">
                <input
                  value={v.name}
                  onChange={(e) => updateRow(idx, { name: e.target.value })}
                  placeholder="navn (i {{…}})"
                  aria-label={`Variabelnavn ${idx + 1}`}
                  className={cn(inputCls, "font-mono text-[13px]")}
                />
                <input
                  value={v.label}
                  onChange={(e) => updateRow(idx, { label: e.target.value })}
                  placeholder="etikett"
                  aria-label={`Variabeletikett ${idx + 1}`}
                  className={inputCls}
                />
              </div>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <select
                  value={v.kind}
                  onChange={(e) => updateRow(idx, { kind: e.target.value })}
                  aria-label={`Variabeltype ${idx + 1}`}
                  className={inputCls}
                >
                  {VAR_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {VAR_KIND_LABELS[k]}
                    </option>
                  ))}
                </select>
                <input
                  value={v.defaultValue}
                  onChange={(e) =>
                    updateRow(idx, { defaultValue: e.target.value })
                  }
                  placeholder="standardverdi"
                  aria-label={`Standardverdi ${idx + 1}`}
                  className={inputCls}
                />
              </div>
              <div className="mt-2 flex items-center justify-between">
                <label className="flex items-center gap-1.5 text-xs text-[var(--color-fg-muted)]">
                  <input
                    type="checkbox"
                    checked={v.required}
                    onChange={(e) =>
                      updateRow(idx, { required: e.target.checked })
                    }
                  />
                  Påkrevd
                </label>
                <button
                  type="button"
                  onClick={() => removeRow(idx)}
                  aria-label={`Fjern variabel ${idx + 1}`}
                  className="rounded-md p-1 text-[var(--color-fg-muted)] hover:text-[var(--color-danger)]"
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Field({
  label,
  required,
  children,
}: {
  label: string;
  required?: boolean;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--color-fg-muted)]">
        {label}
        {required && <span className="text-[var(--color-danger)]"> *</span>}
      </span>
      {children}
    </label>
  );
}
