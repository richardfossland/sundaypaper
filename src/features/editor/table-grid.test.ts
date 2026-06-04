/**
 * table-grid unit tests — the pure grid model behind the `table` block editor.
 * Covers parsing (tolerant of bad/ragged/out-of-range input), serialisation
 * back to the sparse payload, and every grid mutation (set cell, add/remove
 * row & column, header/border toggles). These mirror the contract the Rust
 * render_table relies on (dense grid, dropped out-of-range cells).
 */
import { describe, it, expect } from "vitest";

import {
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
  toPayload,
} from "./table-grid";

describe("parseTableGrid", () => {
  it("seeds a 2x2 starter grid for empty data", () => {
    const g = parseTableGrid("");
    expect(gridSize(g)).toEqual({ rows: 2, cols: 2 });
    expect(g.borders).toBe("all");
    expect(g.headerRow).toBe(false);
    expect(g.rows).toEqual([
      ["", ""],
      ["", ""],
    ]);
  });

  it("falls back to the starter grid for unparseable JSON", () => {
    expect(gridSize(parseTableGrid("not json"))).toEqual({ rows: 2, cols: 2 });
  });

  it("builds a dense grid from a sparse payload, filling missing cells", () => {
    const g = parseTableGrid(
      JSON.stringify({
        numRows: 2,
        numCols: 2,
        cells: [{ rowIndex: 1, colIndex: 1, content: "x" }],
      }),
    );
    expect(g.rows).toEqual([
      ["", ""],
      ["", "x"],
    ]);
  });

  it("drops out-of-range cells (cannot grow the grid)", () => {
    const g = parseTableGrid(
      JSON.stringify({
        numRows: 1,
        numCols: 1,
        cells: [
          { rowIndex: 5, colIndex: 0, content: "off" },
          { rowIndex: 0, colIndex: 0, content: "ok" },
        ],
      }),
    );
    expect(g.rows).toEqual([["ok"]]);
  });

  it("respects explicit zero dimensions", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 0, numCols: 0 }));
    expect(gridSize(g)).toEqual({ rows: 0, cols: 0 });
  });

  it("normalises an unknown border keyword to 'all'", () => {
    const g = parseTableGrid(
      JSON.stringify({ numRows: 1, numCols: 1, borders: "weird" }),
    );
    expect(g.borders).toBe("all");
  });

  it("reads a valid header flag and border keyword", () => {
    const g = parseTableGrid(
      JSON.stringify({
        numRows: 1,
        numCols: 1,
        headerRow: true,
        borders: "none",
      }),
    );
    expect(g.headerRow).toBe(true);
    expect(g.borders).toBe("none");
  });
});

describe("toPayload / serializeTableGrid", () => {
  it("emits only non-empty cells (sparse) with correct dimensions", () => {
    const g: TableGrid = {
      rows: [
        ["a", ""],
        ["", "b"],
      ],
      headerRow: true,
      borders: "outer",
    };
    expect(toPayload(g)).toEqual({
      numRows: 2,
      numCols: 2,
      headerRow: true,
      borders: "outer",
      cells: [
        { rowIndex: 0, colIndex: 0, content: "a" },
        { rowIndex: 1, colIndex: 1, content: "b" },
      ],
    });
  });

  it("round-trips through serialise → parse", () => {
    const g: TableGrid = {
      rows: [
        ["Tid", "Aktivitet"],
        ["11:00", "Velkomst"],
      ],
      headerRow: true,
      borders: "all",
    };
    const back = parseTableGrid(serializeTableGrid(g));
    expect(back).toEqual(g);
  });
});

describe("setCell", () => {
  const g = parseTableGrid(JSON.stringify({ numRows: 2, numCols: 2 }));

  it("writes content at a valid coordinate without mutating the input", () => {
    const next = setCell(g, 0, 1, "hi");
    expect(next.rows[0][1]).toBe("hi");
    expect(g.rows[0][1]).toBe(""); // original untouched (immutable update)
  });

  it("is a no-op for out-of-range coordinates", () => {
    expect(setCell(g, 9, 0, "x")).toBe(g);
    expect(setCell(g, 0, 9, "x")).toBe(g);
    expect(setCell(g, -1, 0, "x")).toBe(g);
  });
});

describe("addRow / addCol", () => {
  it("appends an empty row of the current width", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 1, numCols: 3 }));
    const next = addRow(g);
    expect(gridSize(next)).toEqual({ rows: 2, cols: 3 });
    expect(next.rows[1]).toEqual(["", "", ""]);
  });

  it("appends an empty column to every row", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 2, numCols: 2 }));
    const next = addCol(g);
    expect(gridSize(next)).toEqual({ rows: 2, cols: 3 });
    expect(next.rows.every((r) => r.length === 3)).toBe(true);
  });

  it("addRow on a 0-column grid creates a usable 1-wide row", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 0, numCols: 0 }));
    const next = addRow(g);
    expect(gridSize(next)).toEqual({ rows: 1, cols: 1 });
  });

  it("addCol on a 0-row grid creates a row to hold the column", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 0, numCols: 0 }));
    const next = addCol(g);
    expect(gridSize(next)).toEqual({ rows: 1, cols: 1 });
  });
});

describe("removeRow / removeCol", () => {
  it("removes the row at the given index", () => {
    const g: TableGrid = {
      rows: [["a"], ["b"], ["c"]],
      headerRow: false,
      borders: "all",
    };
    const next = removeRow(g, 1);
    expect(next.rows).toEqual([["a"], ["c"]]);
  });

  it("removes the column at the given index from every row", () => {
    const g: TableGrid = {
      rows: [
        ["a", "b", "c"],
        ["d", "e", "f"],
      ],
      headerRow: false,
      borders: "all",
    };
    const next = removeCol(g, 1);
    expect(next.rows).toEqual([
      ["a", "c"],
      ["d", "f"],
    ]);
  });

  it("is a no-op for an out-of-range index", () => {
    const g = parseTableGrid(JSON.stringify({ numRows: 1, numCols: 1 }));
    expect(removeRow(g, 5)).toBe(g);
    expect(removeCol(g, 5)).toBe(g);
  });
});

describe("setHeaderRow / setBorders", () => {
  const g = parseTableGrid("");
  it("toggles the header flag", () => {
    expect(setHeaderRow(g, true).headerRow).toBe(true);
  });
  it("replaces the border style", () => {
    expect(setBorders(g, "none").borders).toBe("none");
  });
});
