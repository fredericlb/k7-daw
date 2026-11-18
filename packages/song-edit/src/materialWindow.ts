import { stub } from "@studio/song/editStub";
// materialWindow.ts — turn a DAW's (start, length, offset, loop region) into this project's
// model: a WINDOW that belongs to the material and a COUNT that belongs to the placement.
// One rule, shared by the importer and by any migration, so both land on the same numbers.

/** What a foreign DAW says about one placed clip. Everything in beats. */
export interface PlacedExtent {
  length: number;
  offset?: number; // audio: source in-point; midi: playStart
  loopStart?: number;
  loopEnd?: number;
}

/** What this project says instead: a slice of the material, played `loops` times. */
export interface MaterialWindow {
  from: number;
  to: number;
  loops: number;
}

/** The window and how many times it is played — a WHOLE number of loops is "this region, N times". */
export function materialWindow(p: PlacedExtent): MaterialWindow {
  return stub("song-edit/materialWindow.materialWindow", p);
}
