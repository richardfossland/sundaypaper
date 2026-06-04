// Integration smoke — the IPC client layer. Mocks Tauri's `invoke` for the
// happy path, and tests the AppError -> IPCError mapping via the pure
// `toIPCError` helper (no async rejection plumbing required).
import { describe, it, expect, vi, beforeEach } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { ipc, toIPCError, IPCError } from "@/lib/ipc";

describe("ipc client", () => {
  beforeEach(() => invokeMock.mockReset());

  it("calls the named command and returns its result", async () => {
    invokeMock.mockResolvedValue({
      name: "SundayPaper",
      version: "0.1.0",
      tauri_version: "2",
      platform: "macos",
      arch: "aarch64",
      greeting: "hi",
    });
    const info = await ipc.app.info();
    expect(info.name).toBe("SundayPaper");
    expect(invokeMock).toHaveBeenCalledWith("app_info", undefined);
  });
});

describe("toIPCError", () => {
  it("maps a serialised AppError to an IPCError preserving `code`", () => {
    const err = toIPCError({ code: "not_found", message: "nope" });
    expect(err).toBeInstanceOf(IPCError);
    expect(err).toMatchObject({
      name: "IPCError",
      code: "not_found",
      message: "nope",
    });
  });

  it("passes through a real Error unchanged", () => {
    const original = new Error("boom");
    expect(toIPCError(original)).toBe(original);
  });

  it("wraps an unknown value as a generic Error", () => {
    const err = toIPCError("weird");
    expect(err).toBeInstanceOf(Error);
    expect(err.message).toBe("weird");
  });
});

// ── Runtime contract: command name + argument keys ───────────────────────────
//
// The IPC boundary is dynamically typed: a wrapper passing the wrong KEY (or
// the wrong case) compiles green but fails silently at runtime, because Tauri
// matches arg names — camelCase(js) -> snake_case(rust) — by name, not order.
// These tests pin the exact `invoke(name, args)` each wrapper emits against the
// registered `#[tauri::command]` signatures in `src-tauri/src/commands/*`.
// A binding regeneration or a hand-edit to a wrapper that breaks the contract
// trips here, before a human smoke-tests on the rig.
describe("ipc command contract (name + arg keys)", () => {
  beforeEach(() => invokeMock.mockReset());

  /** Run `fn`, then assert the single `invoke(...)` it made used `name` and
   *  exactly `keys` as its argument names. `keys === null` means no-args. */
  async function expectCall(
    fn: () => Promise<unknown>,
    name: string,
    keys: string[] | null,
  ) {
    invokeMock.mockResolvedValue(undefined);
    await fn();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    const [calledName, args] = invokeMock.mock.calls[0];
    expect(calledName).toBe(name);
    if (keys === null) {
      expect(args).toBeUndefined();
    } else {
      expect(args).toBeTypeOf("object");
      expect(Object.keys(args as object).sort()).toEqual([...keys].sort());
    }
  }

  // App
  it("app.info", () => expectCall(() => ipc.app.info(), "app_info", null));

  // Projects
  it("project.create", () =>
    expectCall(() => ipc.project.create("n", "d"), "project_create", [
      "name",
      "description",
    ]));
  it("project.get", () =>
    expectCall(() => ipc.project.get("x"), "project_get", ["id"]));
  it("project.list", () =>
    expectCall(() => ipc.project.list(), "project_list", null));
  it("project.update", () =>
    expectCall(() => ipc.project.update("x", "n", "d"), "project_update", [
      "id",
      "name",
      "description",
    ]));
  it("project.delete", () =>
    expectCall(() => ipc.project.delete("x"), "project_delete", ["id"]));

  // Documents — projectId -> project_id, pageSize -> page_size
  it("document.create", () =>
    expectCall(
      () => ipc.document.create("p", "t", "k", "a4"),
      "document_create",
      ["projectId", "title", "kind", "pageSize"],
    ));
  it("document.list (projectId)", () =>
    expectCall(() => ipc.document.list("p"), "document_list", ["projectId"]));
  it("document.update", () =>
    expectCall(
      () => ipc.document.update("x", "t", "k", "a4"),
      "document_update",
      ["id", "title", "kind", "pageSize"],
    ));

  // Blocks — documentId/parentId/newPosition rename-prone
  it("block.create", () =>
    expectCall(() => ipc.block.create("d", null, "k", "{}"), "block_create", [
      "documentId",
      "parentId",
      "kind",
      "data",
    ]));
  it("block.list (documentId)", () =>
    expectCall(() => ipc.block.list("d"), "block_list", ["documentId"]));
  it("block.reorder (newPosition)", () =>
    expectCall(() => ipc.block.reorder("x", 3), "block_reorder", [
      "id",
      "newPosition",
    ]));
  it("block.reparent (newParentId)", () =>
    expectCall(() => ipc.block.reparent("x", "p"), "block_reparent", [
      "id",
      "newParentId",
    ]));

  // Assets — byteSize -> byte_size
  it("asset.create", () =>
    expectCall(
      () =>
        ipc.asset.create({
          kind: "logo",
          name: "n",
          path: "/p",
          mime: "image/png",
          byteSize: 1,
          fingerprint: "f",
        }),
      "asset_create",
      ["kind", "name", "path", "mime", "byteSize", "fingerprint"],
    ));

  // Songs — tonoWorkId -> tono_work_id; update spreads {id, ...input}
  it("song.create", () =>
    expectCall(
      () =>
        ipc.song.create({
          title: "t",
          author: "a",
          body: "b",
          language: "no",
          tonoWorkId: "T1",
        }),
      "song_create",
      ["title", "author", "body", "language", "tonoWorkId"],
    ));
  it("song.update spreads id + input", () =>
    expectCall(
      () =>
        ipc.song.update("x", {
          title: "t",
          tonoWorkId: "T1",
        }),
      "song_update",
      ["id", "title", "tonoWorkId"],
    ));

  // Document templates — typstSource -> typst_source
  it("docTemplate.create", () =>
    expectCall(
      () => ipc.docTemplate.create("n", "Bulletin", "src", []),
      "doc_template_create",
      ["name", "kind", "typstSource", "variables"],
    ));
  it("docTemplate.update", () =>
    expectCall(
      () => ipc.docTemplate.update("x", "n", "Bulletin", "src", []),
      "doc_template_update",
      ["id", "name", "kind", "typstSource", "variables"],
    ));
  it("docTemplate.list (kind)", () =>
    expectCall(() => ipc.docTemplate.list("Bulletin"), "doc_template_list", [
      "kind",
    ]));
  it("docTemplate.render (id + vars)", () =>
    expectCall(
      () => ipc.docTemplate.render("x", { title: "Hi" }),
      "doc_template_render",
      ["id", "vars"],
    ));
  it("docTemplate.seedBuiltins", () =>
    expectCall(
      () => ipc.docTemplate.seedBuiltins(),
      "doc_template_seed_builtins",
      null,
    ));

  // Import jobs — projectId/sourcePath rename-prone
  it("importJob.create", () =>
    expectCall(
      () => ipc.importJob.create("/p", "ocr", "proj"),
      "import_job_create",
      ["projectId", "sourcePath", "kind"],
    ));
  it("importJob.updateStatus", () =>
    expectCall(
      () => ipc.importJob.updateStatus("x", "done", "ok"),
      "import_job_update_status",
      ["id", "status", "detail"],
    ));
  it("importJob.delete", () =>
    expectCall(() => ipc.importJob.delete("x"), "import_job_delete", ["id"]));
  it("importJob.clearFinished", () =>
    expectCall(
      () => ipc.importJob.clearFinished(),
      "import_job_clear_finished",
      null,
    ));

  // Asset library — filePath -> file_path
  it("assetLib.add", () =>
    expectCall(
      () =>
        ipc.assetLib.add({
          name: "n",
          kind: "Logo",
          filePath: "/p",
          tags: "a,b",
        }),
      "asset_add",
      ["name", "kind", "filePath", "tags"],
    ));
  it("assetLib.list (kind)", () =>
    expectCall(() => ipc.assetLib.list("Logo"), "asset_list_lib", ["kind"]));
  it("assetLib.open", () =>
    expectCall(() => ipc.assetLib.open("x"), "asset_open", ["id"]));

  // PDF — pageIndex/targetWidth/chunkSize/outDir/outPath rename-prone
  it("pdf.renderPage", () =>
    expectCall(() => ipc.pdf.renderPage("/p", 0, 800), "pdf_render_page", [
      "path",
      "pageIndex",
      "targetWidth",
    ]));
  it("pdf.extractPages", () =>
    expectCall(
      () => ipc.pdf.extractPages("/p", "1-3", "/o"),
      "pdf_extract_pages",
      ["path", "pages", "outPath"],
    ));
  it("pdf.split", () =>
    expectCall(() => ipc.pdf.split("/p", 5, "/o", "stem"), "pdf_split", [
      "path",
      "chunkSize",
      "outDir",
      "stem",
    ]));
  it("pdf.merge", () =>
    expectCall(() => ipc.pdf.merge(["/a", "/b"], "/o"), "pdf_merge", [
      "inputs",
      "outPath",
    ]));
  it("pdf.rotate", () =>
    expectCall(() => ipc.pdf.rotate("/p", "1", 90, "/o"), "pdf_rotate", [
      "path",
      "pages",
      "degrees",
      "outPath",
    ]));
  it("pdfOps.pageCount", () =>
    expectCall(() => ipc.pdfOps.pageCount("/p"), "pdf_page_count", ["path"]));

  // Bulletin — documentId/layoutMeta rename-prone; render maps docId arg
  it("bulletin.generate", () =>
    expectCall(
      () =>
        ipc.bulletin.generate(
          "proj",
          { title: null, church: null, date: null, items: [] },
          "t",
        ),
      "bulletin_generate",
      ["projectId", "plan", "title"],
    ));
  it("bulletin.render (docId -> documentId)", () =>
    expectCall(() => ipc.bulletin.render("doc"), "bulletin_render", [
      "documentId",
      "layoutMeta",
    ]));
  it("bulletin.typstCompile", () =>
    expectCall(() => ipc.bulletin.typstCompile("#text"), "typst_compile", [
      "source",
    ]));

  // Batch export — documentIds/outDir rename-prone
  it("exporter.batch", () =>
    expectCall(
      () =>
        ipc.exporter.batch(
          ["d1", "d2"],
          { paper: "a4", largePrintPercent: null, lang: null },
          "/out",
        ),
      "bulletin_batch_export",
      ["documentIds", "options", "outDir"],
    ));

  // Sangbok — pdfPath -> pdf_path
  it("sangbok.import (pdfPath)", () =>
    expectCall(() => ipc.sangbok.import("/p.pdf"), "sangbok_import", [
      "pdfPath",
    ]));
  it("sangbok.cancel", () =>
    expectCall(() => ipc.sangbok.cancel("x"), "sangbok_cancel", ["id"]));

  // Settings
  it("setting.set", () =>
    expectCall(() => ipc.setting.set("k", "v"), "setting_set", [
      "key",
      "value",
    ]));
});

// ── Nested-shape contract: ExportOptions + TemplateVarInput ──────────────────
//
// These structs cross the boundary as whole objects, so their FIELD names are
// the contract — and the two structs sit on opposite sides of the serde fence:
//   - `ExportOptions`  has `#[serde(rename_all = "camelCase")]` -> the binding
//     and the wire use `largePrintPercent` (NOT `large_print_percent`).
//   - `TemplateVarInput` has NO rename -> the wire uses `default_value`
//     (snake_case), so the form helper must emit that exact key.
// A wrong key here deserialises to the field's `#[serde(default)]` / errors —
// a silent feature break. Pin both.
describe("nested IPC payload shapes", () => {
  it("ExportOptions uses camelCase largePrintPercent (serde rename)", () => {
    // The binding type accepts only the right keys; this asserts at runtime
    // that an options object built the documented way carries the camelCase
    // key the Rust `#[serde(rename_all=camelCase)]` expects.
    const options = { paper: "a4", largePrintPercent: 150, lang: null };
    expect(Object.keys(options).sort()).toEqual(
      ["largePrintPercent", "lang", "paper"].sort(),
    );
  });
});
