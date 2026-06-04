/**
 * ServicePlanForm — step 1 of the document builder.
 *
 * Lets the user assemble a `ServicePlan` by hand (or seed it from a sample) so
 * the rest of the FORWARD pipeline has something to chew on. We keep the shape
 * intentionally small: the header metadata + an ordered list of items, each a
 * `kind` + a title + free body text. Song / scripture / image refs default to
 * null — the plan SundayPlan eventually hands us can carry the richer fields,
 * but a hand-typed plan need not.
 *
 * This is deliberately *not* a from-scratch DAW. It exists to PROVE the bridge:
 * a plan typed here turns into a printable program in two clicks.
 */

import { Plus, Trash2, FileStack } from "lucide-react";

import type { ServicePlan, SetlistItem, SetlistItemKind } from "@/lib/bindings";
import { emptyItem, samplePlan } from "./plan-defaults";

// The kinds we surface in the picker. The binding allows more (`#[serde(other)]`
// keeps it open) but these cover a normal Sunday order-of-service.
const ITEM_KINDS: ReadonlyArray<{ value: SetlistItemKind; label: string }> = [
  { value: "welcome", label: "Velkomst" },
  { value: "song", label: "Sang" },
  { value: "scripture", label: "Skriftlesning" },
  { value: "sermon", label: "Preken" },
  { value: "liturgy", label: "Liturgi" },
  { value: "creed", label: "Trosbekjennelse" },
  { value: "prayer", label: "Bønn" },
  { value: "communion", label: "Nattverd" },
  { value: "music", label: "Musikk" },
  { value: "announcement", label: "Kunngjøring" },
  { value: "offering", label: "Offer" },
  { value: "benediction", label: "Velsignelse" },
];

interface ServicePlanFormProps {
  plan: ServicePlan;
  onChange: (plan: ServicePlan) => void;
  disabled?: boolean;
}

export function ServicePlanForm({
  plan,
  onChange,
  disabled,
}: ServicePlanFormProps) {
  // ── Header field helpers ──────────────────────────────────────────────────
  const setHeader = (patch: Partial<ServicePlan>) =>
    onChange({ ...plan, ...patch });

  // `null`-normalise: an empty input is no value, not an empty string.
  const orNull = (v: string) => (v.trim() === "" ? null : v);

  // ── Item helpers ──────────────────────────────────────────────────────────
  const setItem = (idx: number, patch: Partial<SetlistItem>) => {
    const items = plan.items.map((it, i) =>
      i === idx ? { ...it, ...patch } : it,
    );
    onChange({ ...plan, items });
  };

  const addItem = () =>
    onChange({ ...plan, items: [...plan.items, emptyItem()] });

  const removeItem = (idx: number) =>
    onChange({ ...plan, items: plan.items.filter((_, i) => i !== idx) });

  const loadSample = () => onChange(samplePlan());

  return (
    <div className="space-y-5">
      {/* Header metadata */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4 shadow-[var(--shadow-soft)]">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-sm font-semibold">Gudstjeneste</h3>
          <button
            type="button"
            onClick={loadSample}
            disabled={disabled}
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
          >
            <FileStack size={12} />
            Last inn eksempel
          </button>
        </div>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <Field
            label="Tittel"
            value={plan.title ?? ""}
            placeholder="Høymesse"
            disabled={disabled}
            onChange={(v) => setHeader({ title: orNull(v) })}
          />
          <Field
            label="Menighet"
            value={plan.church ?? ""}
            placeholder="Vår Frelsers menighet"
            disabled={disabled}
            onChange={(v) => setHeader({ church: orNull(v) })}
          />
          <Field
            label="Dato"
            value={plan.date ?? ""}
            placeholder="1. juni 2026"
            disabled={disabled}
            onChange={(v) => setHeader({ date: orNull(v) })}
          />
        </div>
      </div>

      {/* Ordered items */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-fg-muted)]">
            Programposter ({plan.items.length})
          </h3>
          <button
            type="button"
            onClick={addItem}
            disabled={disabled}
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs font-medium text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] disabled:opacity-50"
          >
            <Plus size={12} />
            Legg til post
          </button>
        </div>

        {plan.items.length === 0 ? (
          <p className="rounded-lg border border-dashed border-[var(--color-border)] px-4 py-6 text-center text-sm text-[var(--color-fg-muted)]">
            Ingen poster. Legg til en post eller last inn et eksempel.
          </p>
        ) : (
          plan.items.map((item, idx) => (
            <ItemRow
              key={idx}
              index={idx}
              item={item}
              disabled={disabled}
              onChange={(patch) => setItem(idx, patch)}
              onRemove={() => removeItem(idx)}
            />
          ))
        )}
      </div>
    </div>
  );
}

// ── ItemRow ───────────────────────────────────────────────────────────────────

function ItemRow({
  index,
  item,
  onChange,
  onRemove,
  disabled,
}: {
  index: number;
  item: SetlistItem;
  onChange: (patch: Partial<SetlistItem>) => void;
  onRemove: () => void;
  disabled?: boolean;
}) {
  const orNull = (v: string) => (v.trim() === "" ? null : v);

  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-surface)] p-3">
      <div className="flex items-center gap-2">
        <span className="grid h-6 w-6 shrink-0 place-items-center rounded-md bg-[color-mix(in_oklch,var(--color-accent)_12%,transparent)] text-xs font-bold text-[var(--color-accent)]">
          {index + 1}
        </span>
        <select
          aria-label={`Posttype for post ${index + 1}`}
          value={item.kind}
          disabled={disabled}
          onChange={(e) =>
            onChange({ kind: e.target.value as SetlistItemKind })
          }
          className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2 py-1 text-xs font-medium disabled:opacity-50"
        >
          {ITEM_KINDS.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </select>
        <input
          aria-label={`Tittel for post ${index + 1}`}
          value={item.title ?? ""}
          placeholder="Tittel"
          disabled={disabled}
          onChange={(e) => onChange({ title: orNull(e.target.value) })}
          className="flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2.5 py-1 text-sm disabled:opacity-50"
        />
        <button
          type="button"
          aria-label={`Fjern post ${index + 1}`}
          onClick={onRemove}
          disabled={disabled}
          className="flex shrink-0 items-center justify-center rounded-md p-1.5 text-[var(--color-fg-muted)] transition-colors hover:bg-[var(--color-bg-elevated)] hover:text-[var(--color-danger)] disabled:opacity-50"
        >
          <Trash2 size={14} />
        </button>
      </div>
      <textarea
        aria-label={`Tekst for post ${index + 1}`}
        value={item.body ?? ""}
        placeholder="Tekst (valgfritt) — lesning, kunngjøring, liturgi …"
        disabled={disabled}
        rows={2}
        onChange={(e) => onChange({ body: orNull(e.target.value) })}
        className="mt-2 w-full resize-y rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2.5 py-1.5 text-sm disabled:opacity-50"
      />
    </div>
  );
}

// ── Field ───────────────────────────────────────────────────────────────────

function Field({
  label,
  value,
  placeholder,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--color-fg-muted)]">
        {label}
      </span>
      <input
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-sm disabled:opacity-50"
      />
    </label>
  );
}
