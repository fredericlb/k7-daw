// takeEdit.ts — an event's takes on disk: a comp span picked, a take dropped, a take added, and a
// MIDI take written as a sequence of its own. The event is addressed by where it STARTS, as every
// event verb addresses one; the file half is song-schema's takeFile.ts. See packages/song/docs/takes.md.

import { stub } from "@studio/song/editStub";

/** Play `span.take` over `span.from..span.to` (song positions) — the event's own `ref` puts its material back. */
export function setEventComp(
  dir: string,
  track: string,
  eventBeats: number,
  span: { take: string; from: string; to: string },
  num: number,
  den: number,
): string {
  return stub(
    "song-edit/takeEdit.setEventComp",
    dir,
    track,
    eventBeats,
    span,
    num,
    den,
  );
}

/** Set the crossfade at the comp seam `at` (a song position), `length` a duration; null removes it. */
export function setEventCrossfade(
  dir: string,
  track: string,
  eventBeats: number,
  at: string,
  length: string | null,
  num: number,
  den: number,
): string {
  return stub(
    "song-edit/takeEdit.setEventCrossfade",
    dir,
    track,
    eventBeats,
    at,
    length,
    num,
    den,
  );
}

/** Drop take `ref` and every span it played; its material stays on disk, as a deleted clip's does. */
export function removeEventTake(
  dir: string,
  track: string,
  eventBeats: number,
  ref: string,
  num: number,
  den: number,
): string {
  return stub(
    "song-edit/takeEdit.removeEventTake",
    dir,
    track,
    eventBeats,
    ref,
    num,
    den,
  );
}

/** Add take `ref` starting at `start`, heard over `span` (null: kept but not heard). */
export function addEventTake(
  dir: string,
  track: string,
  eventBeats: number,
  take: { ref: string; start: string },
  span: { from: string; to: string } | null,
  num: number,
  den: number,
): string {
  return stub(
    "song-edit/takeEdit.addEventTake",
    dir,
    track,
    eventBeats,
    take,
    span,
    num,
    den,
  );
}

/** A recorded MIDI pass as `sequences/<id>.k7x`, notes in beats from the take's own start, its window the whole pass. */
export function writeTakeSequence(
  dir: string,
  id: string,
  notes: { t: number; d: number; k: number; v?: number }[],
  lengthBeats: number,
  num: number,
  den: number,
): string {
  return stub(
    "song-edit/takeEdit.writeTakeSequence",
    dir,
    id,
    notes,
    lengthBeats,
    num,
    den,
  );
}
