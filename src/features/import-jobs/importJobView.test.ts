import { describe, it, expect } from "vitest";

import type { ImportJob } from "@/lib/bindings";
import {
  baseName,
  isFinished,
  kindLabel,
  normStatus,
  statusCounts,
  statusLabel,
  viewJobs,
} from "./importJobView";

const mkJob = (over: Partial<ImportJob>): ImportJob => ({
  id: "j-1",
  project_id: null,
  source_path: "/tmp/scan.pdf",
  kind: "ocr",
  status: "pending",
  detail: null,
  created_at: 0n,
  updated_at: 0n,
  ...over,
});

describe("importJobView", () => {
  it("normalises status case-insensitively, defaulting unknown to pending", () => {
    expect(normStatus("DONE")).toBe("done");
    expect(normStatus("Running")).toBe("running");
    expect(normStatus("error")).toBe("error");
    expect(normStatus("queued")).toBe("pending");
  });

  it("labels statuses and kinds in Norwegian", () => {
    expect(statusLabel("done")).toBe("Ferdig");
    expect(statusLabel("error")).toBe("Feilet");
    expect(kindLabel("ocr")).toBe("OCR");
    expect(kindLabel("weird")).toBe("weird");
  });

  it("treats done/error as finished, pending/running as not", () => {
    expect(isFinished("done")).toBe(true);
    expect(isFinished("error")).toBe(true);
    expect(isFinished("pending")).toBe(false);
    expect(isFinished("running")).toBe(false);
  });

  it("extracts the base file name from posix and windows paths", () => {
    expect(baseName("/a/b/scan.pdf")).toBe("scan.pdf");
    expect(baseName("C:\\docs\\merge.pdf")).toBe("merge.pdf");
    expect(baseName("loose.pdf")).toBe("loose.pdf");
  });

  it("sorts jobs newest-first and can hide finished ones", () => {
    const older = mkJob({ id: "old", created_at: 100n, status: "done" });
    const newer = mkJob({ id: "new", created_at: 200n, status: "running" });
    const sorted = viewJobs([older, newer], { hideFinished: false });
    expect(sorted.map((j) => j.id)).toEqual(["new", "old"]);

    const filtered = viewJobs([older, newer], { hideFinished: true });
    expect(filtered.map((j) => j.id)).toEqual(["new"]);
  });

  it("does not mutate the input array", () => {
    const input = [
      mkJob({ id: "a", created_at: 1n }),
      mkJob({ id: "b", created_at: 2n }),
    ];
    const copy = [...input];
    viewJobs(input, { hideFinished: false });
    expect(input).toEqual(copy);
  });

  it("counts jobs per status bucket", () => {
    const counts = statusCounts([
      mkJob({ status: "done" }),
      mkJob({ status: "done" }),
      mkJob({ status: "error" }),
      mkJob({ status: "pending" }),
    ]);
    expect(counts).toEqual({ pending: 1, running: 0, done: 2, error: 1 });
  });
});
