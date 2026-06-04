/**
 * TableEditor — a structured grid editor for the `table` block kind.
 *
 * Rather than hand-editing the sparse `{numRows,numCols,cells,...}` JSON in a
 * textarea, the volunteer edits a real grid: type into cells, add/remove rows
 * and columns, toggle the header row and the border style. All grid logic lives
 * in table-grid.ts (pure + unit-tested); this component only wires events to it
 * and reports the serialised payload back up via `onChange`.
 *
 * It is fully controlled: it derives its working grid from the incoming `data`
 * string and emits a fresh serialised string on every edit, so BlockCard's
 * existing dirty/save flow keeps working unchanged.
 */

import { useMemo } from "react";
import { Plus, Trash2 } from "lucide-react";

import { cn } from "@/lib/cn";
import {
  type TableBorders,
  type TableGrid,
  addCol,
  addRow,
  gridSize,
  parseTableGrid,
  removeCol,
  removeRow,
  serializeTableGrid,
  setBorders,
  setCell,
  setHeaderRow,
} from "./table-grid";

interface TableEditorProps {
  /** The block's persisted `data` JSON string. */
  data: string;
  busy: boolean;
  /** Called with a fresh serialised payload string after any edit. */
  onChange: (next: string) => void;
}

const BORDER_LABELS: Record<TableBorders, string> = {
  all: "Alle linjer",
  outer: "Bare ramme",
  none: "Ingen linjer",
};

export function TableEditor({ data, busy, onChange }: TableEditorProps) {
  const grid = useMemo(() => parseTableGrid(data), [data]);
  const { rows, cols } = gridSize(grid);

  const emit = (next: TableGrid) => onChange(serializeTableGrid(next));

  return (
    <div className="mt-2 space-y-2">
      {/* Options row: header toggle + border style. */}
      <div className="flex flex-wrap items-center gap-3 text-xs">
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            aria-label="Overskriftsrad"
            checked={grid.headerRow}
            disabled={busy}
            onChange={(e) => emit(setHeaderRow(grid, e.target.checked))}
          />
          Overskriftsrad
        </label>

        <label className="flex items-center gap-1.5">
          Kantlinjer
          <select
            aria-label="Kantlinjer"
            value={grid.borders}
            disabled={busy}
            onChange={(e) =>
              emit(setBorders(grid, e.target.value as TableBorders))
            }
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-1.5 py-0.5"
          >
            {(["all", "outer", "none"] as const).map((b) => (
              <option key={b} value={b}>
                {BORDER_LABELS[b]}
              </option>
            ))}
          </select>
        </label>

        <span className="flex-1" />

        <button
          type="button"
          disabled={busy}
          onClick={() => emit(addRow(grid))}
          className="flex items-center gap-1 rounded-md border border-[var(--color-border)] px-2 py-0.5 hover:bg-[var(--color-bg-elevated)] disabled:opacity-40"
        >
          <Plus size={12} /> Rad
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => emit(addCol(grid))}
          className="flex items-center gap-1 rounded-md border border-[var(--color-border)] px-2 py-0.5 hover:bg-[var(--color-bg-elevated)] disabled:opacity-40"
        >
          <Plus size={12} /> Kolonne
        </button>
      </div>

      {rows === 0 || cols === 0 ? (
        <p className="text-xs text-[var(--color-fg-muted)]">
          Tom tabell. Legg til en rad og en kolonne for å begynne.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table className="border-collapse text-xs">
            <tbody>
              {grid.rows.map((row, r) => (
                <tr key={r}>
                  {row.map((cell, c) => (
                    <td
                      key={c}
                      className="border border-[var(--color-border)] p-0.5"
                    >
                      <input
                        type="text"
                        aria-label={`Celle rad ${r + 1} kolonne ${c + 1}`}
                        value={cell}
                        disabled={busy}
                        onChange={(e) =>
                          emit(setCell(grid, r, c, e.target.value))
                        }
                        className={cn(
                          "w-28 bg-transparent px-1.5 py-1 outline-none",
                          grid.headerRow && r === 0 && "font-semibold",
                        )}
                      />
                    </td>
                  ))}
                  <td className="pl-1">
                    <button
                      type="button"
                      aria-label={`Slett rad ${r + 1}`}
                      disabled={busy}
                      onClick={() => emit(removeRow(grid, r))}
                      className="rounded p-0.5 text-[var(--color-fg-muted)] hover:text-[var(--color-danger)] disabled:opacity-40"
                    >
                      <Trash2 size={12} />
                    </button>
                  </td>
                </tr>
              ))}
              {/* Footer row of column-delete buttons, aligned under each column. */}
              <tr>
                {grid.rows[0].map((_, c) => (
                  <td key={c} className="text-center">
                    <button
                      type="button"
                      aria-label={`Slett kolonne ${c + 1}`}
                      disabled={busy}
                      onClick={() => emit(removeCol(grid, c))}
                      className="rounded p-0.5 text-[var(--color-fg-muted)] hover:text-[var(--color-danger)] disabled:opacity-40"
                    >
                      <Trash2 size={12} />
                    </button>
                  </td>
                ))}
                <td />
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
