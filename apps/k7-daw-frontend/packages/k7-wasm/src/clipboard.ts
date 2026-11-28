// clipboard.ts — what `selection copy` puts aside and `selection paste` puts down: a rectangle's contents, in coordinates RELATIVE to its top-left corner.
// A reader would get wrong that a note or an event is IN a rectangle when it STARTS inside: copying and deleting share `notesInRegion`.

import { call } from "./abi.ts";
import type { Clipboard, EventAttrs } from "./types/index.ts";

export * from "./ops/clipboard.ts";

export type {
  Clipboard,
  EventAttrs,
  PastedEvent,
  PastedNote,
} from "./types/index.ts";

/** Nothing to put down: an empty rectangle copies as `null` rather than as an empty clipboard. */
export type MaybeClipboard = Clipboard | null;

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `clipboard.${op}`, args });

/** What a rectangle HOLDS: a note is in when it STARTS inside. */
export function notesInRegion<T extends { key: number; time: number }>(
  notes: readonly T[],
  loKey: number,
  hiKey: number,
  from: number,
  to: number,
): T[] {
  const at = ask<number[]>("in_region", {
    notes: notes.map((n) => [n.key, n.time]),
    loKey,
    hiKey,
    from,
    to,
  });
  return at.map((i) => notes[i]);
}

/** The notes inside the rectangle, relative to its corner; null for an empty rectangle. */
export const copyNotes = (
  notes: readonly {
    key: number;
    time: number;
    duration: number;
    vel?: number;
    repeat?: number;
    skip?: number;
  }[],
  sequence: string,
  loKey: number,
  hiKey: number,
  from: number,
  to: number,
): MaybeClipboard =>
  ask("copy_notes", {
    notes: notes.map(({ key, time, duration, vel, repeat, skip }) => ({
      key,
      time,
      duration,
      vel,
      repeat,
      skip,
    })),
    sequence,
    loKey,
    hiKey,
    from,
    to,
  });

/** The events inside a rectangle of lanes × beats, relative to its corner, taken by START like the notes. */
export const copyEvents = (
  rows: readonly {
    events: readonly { ref: string; start: number; event?: EventAttrs }[];
  }[],
  from: number,
  to: number,
): MaybeClipboard =>
  ask("copy_events", {
    rows: rows.map((r) => ({
      events: r.events.map(({ ref, start, event }) => ({ ref, start, event })),
    })),
    from,
    to,
  });
