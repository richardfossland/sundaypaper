/**
 * Pure grid model + operations for the `table` block kind.
 *
 * The layout engine's `render_table` (see services/layout/markup.rs) consumes a
 * payload shaped like:
 *
 *   { numRows, numCols, cells: [{rowIndex, colIndex, content}], headerRow, borders }
 *
 * The persisted block `data` is a JSON string of exactly that shape. Editing it
 * by hand in a textarea is miserable, so the editor works on a dense 2-D string
 * grid instead and serialises back to the sparse payload on save. All of that
 * logic lives here — framework-free and exhaustively unit-tested — so the React
 * component (TableEditor.tsx) only wires events to these functions.
 */

/** A single non-empty cell in the persisted, sparse payload. */
export interface TableCellSpec {
  rowIndex: number;
  colIndex: number;
  content: string;
}

/** Border style keyword the renderer accepts. */
export type TableBorders = "all" | "outer" | "none";

/** The persisted `table` block payload (mirrors render_table's expectations). */
export interface TablePayload {
  numRows: number;
  numCols: number;
  cells: TableCellSpec[];
  headerRow: boolean;
  borders: TableBorders;
}

/** The editor's working model: dimensions + a dense row-major cell grid. */
export interface TableGrid {
  rows: string[][]; // rows[r][c]
  headerRow: boolean;
  borders: TableBorders;
}

const VALID_BORDERS: readonly TableBorders[] = ["all", "outer", "none"];

/** Clamp/normalise a dimension to a non-negative integer (defensive). */
function dim(n: unknown): number {
  const v = typeof n === "number" && Number.isFinite(n) ? Math.floor(n) : 0;
  return v < 0 ? 0 : v;
}

/** Build an empty `rows × cols` dense grid of empty strings. */
function emptyRows(rows: number, cols: number): string[][] {
  return Array.from({ length: rows }, () =>
    Array.from({ length: cols }, () => ""),
  );
}

/**
 * Parse a persisted `data` JSON string into a dense {@link TableGrid}. Tolerant
 * of anything: bad JSON, missing fields, ragged/out-of-range/duplicate cells —
 * it always yields a well-formed dense grid (last write wins for a duplicated
 * cell; out-of-range cells are dropped). A brand-new table (empty data) seeds a
 * sensible 2×2 starter so the editor is immediately usable.
 */
export function parseTableGrid(raw: string): TableGrid {
  let obj: Record<string, unknown> = {};
  if (raw.trim() !== "") {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (parsed && typeof parsed === "object")
        obj = parsed as Record<string, unknown>;
    } catch {
      // fall through to defaults
    }
  }

  const hasDims = "numRows" in obj || "numCols" in obj;
  const rows = hasDims ? dim(obj.numRows) : 2;
  const cols = hasDims ? dim(obj.numCols) : 2;

  const grid = emptyRows(rows, cols);
  const cells = Array.isArray(obj.cells) ? obj.cells : [];
  for (const c of cells) {
    if (!c || typeof c !== "object") continue;
    const cell = c as Record<string, unknown>;
    const r = dim(cell.rowIndex);
    const col = dim(cell.colIndex);
    if (r >= rows || col >= cols) continue;
    grid[r][col] = typeof cell.content === "string" ? cell.content : "";
  }

  const borders = VALID_BORDERS.includes(obj.borders as TableBorders)
    ? (obj.borders as TableBorders)
    : "all";

  return { rows: grid, headerRow: obj.headerRow === true, borders };
}

/** Current dimensions of a grid (0×0 when empty). */
export function gridSize(g: TableGrid): { rows: number; cols: number } {
  return { rows: g.rows.length, cols: g.rows[0]?.length ?? 0 };
}

/** Serialise a dense grid back into the sparse persisted payload (drops empty
 * cells so the JSON stays small) and then to a JSON string. */
export function serializeTableGrid(g: TableGrid): string {
  return JSON.stringify(toPayload(g));
}

/** Dense grid → sparse {@link TablePayload}. Exposed for tests. */
export function toPayload(g: TableGrid): TablePayload {
  const { rows, cols } = gridSize(g);
  const cells: TableCellSpec[] = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const content = g.rows[r][c];
      if (content !== "") cells.push({ rowIndex: r, colIndex: c, content });
    }
  }
  return {
    numRows: rows,
    numCols: cols,
    cells,
    headerRow: g.headerRow,
    borders: g.borders,
  };
}

/** Return a new grid with `content` written at (r, c). No-op if out of range. */
export function setCell(
  g: TableGrid,
  r: number,
  c: number,
  content: string,
): TableGrid {
  const { rows, cols } = gridSize(g);
  if (r < 0 || r >= rows || c < 0 || c >= cols) return g;
  const next = g.rows.map((row) => row.slice());
  next[r][c] = content;
  return { ...g, rows: next };
}

/** Append an empty row at the bottom. */
export function addRow(g: TableGrid): TableGrid {
  const { cols } = gridSize(g);
  // A grid with zero columns still grows by one (empty) row so the user can
  // then add columns; keep at least one column for a usable new row.
  const width = cols === 0 ? 1 : cols;
  const next = g.rows.map((row) => row.slice());
  // If we just gave the table its first column, widen existing rows too.
  if (cols === 0) {
    for (const row of next) row.push("");
  }
  next.push(Array.from({ length: width }, () => ""));
  return { ...g, rows: next };
}

/** Append an empty column to every row. */
export function addCol(g: TableGrid): TableGrid {
  const { rows } = gridSize(g);
  // An empty (0-row) grid gains its first row so a column has somewhere to live.
  const base = rows === 0 ? [[]] : g.rows.map((row) => row.slice());
  for (const row of base) row.push("");
  return { ...g, rows: base };
}

/** Remove the row at index `r`. No-op if out of range. */
export function removeRow(g: TableGrid, r: number): TableGrid {
  const { rows } = gridSize(g);
  if (r < 0 || r >= rows) return g;
  return { ...g, rows: g.rows.filter((_, i) => i !== r) };
}

/** Remove the column at index `c` from every row. No-op if out of range. */
export function removeCol(g: TableGrid, c: number): TableGrid {
  const { cols } = gridSize(g);
  if (c < 0 || c >= cols) return g;
  const next = g.rows.map((row) => row.filter((_, i) => i !== c));
  // Dropping the last column leaves rows that are now zero-width; that is a
  // valid (degenerate) grid the renderer treats as 0-dim.
  return { ...g, rows: next };
}

/** Toggle/replace the header-row flag. */
export function setHeaderRow(g: TableGrid, headerRow: boolean): TableGrid {
  return { ...g, headerRow };
}

/** Replace the border style. */
export function setBorders(g: TableGrid, borders: TableBorders): TableGrid {
  return { ...g, borders };
}
