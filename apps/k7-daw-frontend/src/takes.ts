// takes.ts — what the TAKES view of the waveform and piano roll panels draws, off the open project: the event a clip belongs to, its takes as rows, and which spans of each row the comp plays.
// The rules are Rust's (song/src/reads/takes.rs): an answer names pieces by index, and this file hands back the page's own clips.
// See apps/k7-daw-wasm/song/docs/takes.md and docs/clipEditors.md.

import {
  armLine as arm,
  pieceEdgeLines as edgeLines,
  compFadeLine as fadeLine,
  heardSpans as heard,
  compLine as line,
  type TakesEvent as Read,
  compSeams as seams,
  type TakeRow,
  takesEvent,
} from "@studio/k7-wasm/reads";
import { SERIES_COLORS, seriesColor } from "@studio/ui/series-colors";
import type { WebClip, WebProject } from "./types";

export type { TakeRow };

/** An event with its takes, everything in song BEATS; `pieces` is what the engine plays of it, in order — the folded clip's pieces, or the event's one clip when it is not comped. */
export interface TakesEvent extends Omit<Read, "pieces" | "pieceHost"> {
  pieces: WebClip[];
}

/** The event a clip belongs to: a piece of a comp names it, an uncomped clip IS it — stable across a re-comp. */
export const eventIdOf = (clip: Pick<WebClip, "id" | "comp">): string =>
  clip.comp?.of ?? clip.id;

/** The event `eventId` on `track`, with its takes and comp — null when no clip of it is on that lane. */
export function takesEventOf(
  project: WebProject,
  track: string,
  eventId: string,
): TakesEvent | null {
  const lane = project.tracks.find((t) => t.id === track);
  if (!lane) return null;
  const read = takesEvent(track, eventId, lane.clips, {
    num: project.timeSigNumerator,
    den: project.timeSigDenominator,
  });
  if (!read) return null;
  const { pieces, pieceHost, ...ev } = read;
  const from =
    pieceHost === undefined ? lane.clips : (lane.clips[pieceHost].pieces ?? []);
  return {
    ...ev,
    pieces: pieces.map((i) => from[i]),
  };
}

/** The spans of song time row `ref` is HEARD over: its comp spans, and for the event's own, what no span covers. */
export const heardSpans = (
  ev: TakesEvent,
  row: TakeRow,
  rowEnd: number,
): { from: number; to: number }[] => heard(ev.comp, row, rowEnd);

/** The line that plays `ref` over `span` — the one command a pick is. */
export const compLine = (
  ev: TakesEvent,
  ref: string,
  span: { from: number; to: number },
  num: number,
  den: number,
): string => line(ev.track, ev.at, ref, span.from, span.to, { num, den });

/** The line that arms the next take into `ev`: punched into the selection, or from the playhead to the stop. */
export const armLine = (
  ev: TakesEvent,
  from: number,
  to: number | null,
  num: number,
  den: number,
): string => arm(ev.track, ev.at, from, to, { num, den });

/** Where two pieces of the comp meet inside the event — the places a crossfade can go. */
export const compSeams = (ev: TakesEvent): number[] =>
  seams(ev.start, ev.to, ev.comp);

/** The line that crossfades the seam `seam` over `beats` — 0 removes it. */
export const compFadeLine = (
  ev: TakesEvent,
  seam: number,
  beats: number,
  num: number,
  den: number,
): string => fadeLine(ev.track, ev.at, seam, beats, { num, den });

/** A dragged span, snapped to `grid` beats and put in order — null for a click, which seeks rather than selects. */
export function snapSpan(
  a: number,
  b: number,
  grid: number,
): { from: number; to: number } | null {
  const snap = (x: number) => Math.round(x / grid) * grid;
  const from = snap(Math.min(a, b));
  const to = snap(Math.max(a, b));
  return to - from >= grid ? { from, to } : null;
}

/** The number a take's row shows: its place among the takes, the event's own row not counted, from 1. */
export function takeNumber(rows: readonly TakeRow[], i: number): number {
  return rows.slice(0, i + 1).filter((r) => !r.own).length;
}

/** One colour per take, by its row: the event's own first. The same in the takes view and the comp strip, and the studio's one series palette. */
export const TAKE_COLORS: readonly string[] = SERIES_COLORS.map((_, i) =>
  seriesColor(i),
);

export function takeColor(ev: TakesEvent, ref: string): string {
  const i = Math.max(
    0,
    ev.rows.findIndex((r) => r.ref === ref),
  );
  return TAKE_COLORS[i % TAKE_COLORS.length];
}

/** The take a piece plays: its comp marker, else its own material. */
export const takeOfPiece = (p: WebClip): string => p.comp?.take || p.name;

/** A piece's edges dragged to `from..to` (song beats), as comp lines: grown, its take takes the span; shrunk, the neighbour takes back what it let go. */
export const pieceEdgeLines = (
  ev: TakesEvent,
  piece: WebClip,
  from: number,
  to: number,
  num: number,
  den: number,
): string[] =>
  edgeLines(ev.track, ev.at, ev.ref, ev.pieces, piece.id, from, to, {
    num,
    den,
  });

/** One bar per bucket, mirrored about the middle — the envelope as the waveform panel draws it, in beats from `start`. */
export function envelopePath(
  rms: number[],
  length: number,
  start: number,
): string {
  const step = length / Math.max(1, rms.length);
  // A loop, not `Math.max(...rms)`: a spread of a long envelope is an argument list the engine may refuse.
  let peak = 1e-6;
  for (const v of rms) if (v > peak) peak = v;
  let d = "";
  for (let i = 0; i < rms.length; i++) {
    const h = Math.min(1, rms[i] / peak) * 0.45;
    d += `M${start + i * step} ${0.5 - h}h${step}v${2 * h}h${-step}Z`;
  }
  return d;
}
