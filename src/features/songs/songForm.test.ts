import { describe, it, expect } from "vitest";

import type { Song } from "@/lib/bindings";
import {
  emptySongForm,
  formToPayload,
  isSongFormValid,
  songToForm,
} from "./songForm";

const mkSong = (over: Partial<Song>): Song => ({
  id: "s-1",
  title: "Navn over alle navn",
  author: "Ukjent",
  body: "vers 1\nvers 2",
  language: "no",
  tono_work_id: "T-123",
  created_at: 0n,
  updated_at: 0n,
  deleted_at: null,
  ...over,
});

describe("songForm", () => {
  it("seeds the form from a song, nulls become empty strings", () => {
    const form = songToForm(
      mkSong({ author: null, language: null, tono_work_id: null }),
    );
    expect(form).toEqual({
      title: "Navn over alle navn",
      author: "",
      body: "vers 1\nvers 2",
      language: "",
      tonoWorkId: "",
    });
  });

  it("requires a non-blank title to be valid", () => {
    expect(isSongFormValid(emptySongForm)).toBe(false);
    expect(isSongFormValid({ ...emptySongForm, title: "   " })).toBe(false);
    expect(isSongFormValid({ ...emptySongForm, title: "Salme" })).toBe(true);
  });

  it("trims fields and drops blank optionals to undefined", () => {
    const payload = formToPayload({
      title: "  Salme  ",
      author: "  ",
      body: "  tekst  ",
      language: "",
      tonoWorkId: "  T-9  ",
    });
    expect(payload).toEqual({
      title: "Salme",
      author: undefined,
      body: "tekst",
      language: undefined,
      tonoWorkId: "T-9",
    });
  });
});
