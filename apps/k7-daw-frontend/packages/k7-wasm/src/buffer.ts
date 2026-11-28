// buffer.ts — what the editor reads off the open buffer's text: its reads, the cursor's place, a scalar field, the tokens, the ▶ lines, a clip's window.
// A reader would get wrong that a span is the TREE's: map it through the edits made since the text was parsed before it is used (ui-editor's tree.ts).

import { call } from "./abi.ts";
import type {
  BufferReads,
  ClipGrid,
  K7xCursor,
  KeyToken,
  PreviewTrigger,
  SliceSym,
  ValueToken,
} from "./types/index.ts";

export type {
  BufferReads,
  ClipGrid,
  K7xCursor,
  KeyToken,
  PreviewReq,
  PreviewTrigger,
  SliceSym,
  ValueToken,
} from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `buffer.${op}`, args });

export const NO_READS: BufferReads = {
  slices: false,
  io: null,
  channels: null,
};

/** The three reads (a slice rack, an input's io, its channels): one parse, and none when the text cannot hold the keys they look for. */
export const readBuffer = (text: string, path: string): BufferReads =>
  ask("reads", { text, path });

/** Where the cursor sits in key terms, read from the text BEFORE it by a tolerant scan: completion runs on half-written text. */
export const k7xCursor = (text: string): K7xCursor => ask("cursor", { text });

/** A string field's value, a call's `key("v")` or an object's `key: "v"`, found by a scan so a half-typed file still answers. */
export const readScalarField = (text: string, key: string): string | null =>
  ask("scalar", { text, key });

/** Every non-empty string value of the tree with its field; empty for a text that does not read. */
export const valueTokensOf = (text: string): ValueToken[] =>
  ask("value_tokens", { text });

/** Every non-empty map key of the tree with the keys above it; empty for a text that does not read. */
export const keyTokensOf = (text: string): KeyToken[] =>
  ask("key_tokens", { text });

/** The lines that earn a ▶, spans in the tree's coordinates, a slice after the source it belongs to. */
export const previewTriggersOf = (text: string): PreviewTrigger[] =>
  ask("preview_triggers", { text });

/** Each slice symbol's window and where its numbers and name sit; null for a file that is no slice rack or does not parse. */
export const scanSlices = (text: string): SliceSym[] | null =>
  ask("slices", { text });

/** The window as ONE band to draw over the source: HOLLOW with neither edge written, since the edges are still the HANDLES. */
export function clipWindowBand(
  text: string,
  grid: ClipGrid,
  editable = false,
):
  | { symbol: string; from: number; to: number; hollow?: boolean }[]
  | undefined {
  const band = ask<{
    from: number;
    to: number | null;
    hollow: boolean;
  } | null>("clip_band", { text, grid, editable });
  if (!band) return undefined;
  const to = band.to ?? Number.POSITIVE_INFINITY;
  return [
    band.hollow
      ? { symbol: "clip", from: band.from, to, hollow: true }
      : { symbol: "clip", from: band.from, to },
  ];
}
