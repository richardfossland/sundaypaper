/**
 * Pure helpers for the Song editor form. Kept free of React so the
 * normalisation + validation rules can be unit-tested without rendering.
 */

import type { Song } from "@/lib/bindings";

/** The editable fields of a song, as plain strings the form binds to. */
export interface SongFormState {
  title: string;
  author: string;
  body: string;
  language: string;
  tonoWorkId: string;
}

/** The payload shape `ipc.song.create` / `ipc.song.update` expect. */
export interface SongPayload {
  title: string;
  author?: string;
  body?: string;
  language?: string;
  tonoWorkId?: string;
}

/** A blank form, used when composing a new song. */
export const emptySongForm: SongFormState = {
  title: "",
  author: "",
  body: "",
  language: "",
  tonoWorkId: "",
};

/** Seed the form from an existing song (nulls become empty strings). */
export function songToForm(song: Song): SongFormState {
  return {
    title: song.title,
    author: song.author ?? "",
    body: song.body,
    language: song.language ?? "",
    tonoWorkId: song.tono_work_id ?? "",
  };
}

/** A song is saveable only with a non-blank title. */
export function isSongFormValid(form: SongFormState): boolean {
  return form.title.trim().length > 0;
}

/**
 * Turn the form into an IPC payload: trim everything and drop blank optional
 * fields to `undefined` so the backend stores `NULL` rather than empty strings.
 */
export function formToPayload(form: SongFormState): SongPayload {
  const opt = (v: string) => {
    const t = v.trim();
    return t.length > 0 ? t : undefined;
  };
  return {
    title: form.title.trim(),
    author: opt(form.author),
    body: opt(form.body),
    language: opt(form.language),
    tonoWorkId: opt(form.tonoWorkId),
  };
}
