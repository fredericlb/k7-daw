// media.ts — media bytes from Rust: a WebM's duration written into its head, a take's trim window, a waveform tile's colours.
// A reader would get wrong that bytes cross as base64: a head is 64 kB at most and a strip a few kilobytes, once per take or tile.

import { call } from "./abi.ts";

export { DEFAULT_TRIM_DB } from "./consts.generated.ts";
export * from "./ops/media.ts";
export type { TimeSpan } from "./types/index.ts";

const toBase64 = (bytes: Uint8Array): string => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
};

const fromBase64 = (text: string): Uint8Array =>
  Uint8Array.from(atob(text), (c) => c.charCodeAt(0));

/** `bytes` replaces the first `consumed` bytes of the take; consumed 0 means it already had a duration. */
export interface WebmPatch {
  bytes: Uint8Array;
  consumed: number;
}

/** A take's head with its Duration written, or null when its layout is not one the patcher knows. */
export function patchWebmHead(
  head: Uint8Array,
  durationMs: number,
): WebmPatch | null {
  const a = call<{ bytes: string; consumed: number } | null>({
    op: "media.webmPatch",
    args: { head: toBase64(head), durationMs },
  });
  return a ? { bytes: fromBase64(a.bytes), consumed: a.consumed } : null;
}

/** One column's low, mid and high peaks, or null where the file has no bands. */
export type BandColumn = readonly [number, number, number] | null;

/** An RGBA strip, four bytes a column, opaque; a column with no bands takes `plain`. */
export function spectrumStrip(
  columns: readonly BandColumn[],
  plain: readonly [number, number, number],
): Uint8ClampedArray {
  const text = call<string>({
    op: "media.spectrumStrip",
    args: { columns, plain },
  });
  return new Uint8ClampedArray(fromBase64(text));
}
