// key.ts — the Camelot wheel: where a key sits on the DJ's wheel, the one part of key detection a page reads.
// The detector itself is not here; see ../docs/key.md.

/** The twelve, C first — the order a tonic index counts in. */
export const PITCH_CLASSES = [
  "C",
  "C#",
  "D",
  "D#",
  "E",
  "F",
  "F#",
  "G",
  "G#",
  "A",
  "A#",
  "B",
] as const;

export type Mode = "major" | "minor";

/** A key, as the desk would say it: `A#` + `minor`, and how far ahead of the runner-up it was. */
export interface DetectedKey {
  /** 0..11, C = 0. */
  tonic: number;
  mode: Mode;
  margin: number;
}

/** Where a key sits on the Camelot wheel: `8B` for C major, `8A` for A minor. */
export function camelot(key: DetectedKey | null): string {
  if (!key) return "";
  // The wheel walks in fifths, so the hour is the tonic's position in the circle of fifths.
  const FIFTHS_MAJOR = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
  const FIFTHS_MINOR = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];
  const hour =
    key.mode === "major" ? FIFTHS_MAJOR[key.tonic] : FIFTHS_MINOR[key.tonic];
  return `${hour}${key.mode === "major" ? "B" : "A"}`;
}
