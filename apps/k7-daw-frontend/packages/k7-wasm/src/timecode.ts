// timecode.ts — "bar:beat:eighth:sixteenth.ticks" timecodes <-> beats, answered by the Rust crate (song-schema's timecode.rs).
// A reader would get wrong that these wait for the module: they are sync, and throw `unavailable` before wasmReady() resolves.
// POSITIONS ("1:1:1:1") are 1-based, DURATIONS ("1:0:0:0") 0-based (apps/k7-daw-wasm/song-schema/docs/timecode.md).

import { call, roundBeatHot } from "./abi.ts";

export { TICKS_PER_QUARTER, TIMECODE } from "./consts.generated.ts";

const parse = (tc: string, num: number, den: number, position: boolean) =>
  call<number>({ op: "timecode.parse", args: { tc, num, den, position } });

const format = (beats: number, num: number, den: number, position: boolean) =>
  call<string>({ op: "timecode.format", args: { beats, num, den, position } });

/** A 1-based timeline or clip-relative position, in beats; a bad one throws `bad_timecode`. */
export const parsePosition = (tc: string, num: number, den: number): number =>
  parse(tc, num, den, true);

/** A 0-based duration (length, offset, fade), in beats. */
export const parseDuration = (tc: string, num: number, den: number): number =>
  parse(tc, num, den, false);

export const beatsToPosition = (
  beats: number,
  num: number,
  den: number,
): string => format(beats, num, den, true);

export const beatsToDuration = (
  beats: number,
  num: number,
  den: number,
): string => format(beats, num, den, false);

/** A beat rounded to six decimals, so a float's tail never reaches a file or a key. */
export const roundBeat = (beats: number): number => roundBeatHot(beats);
