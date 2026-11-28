// buffer.ts — what the editor reads off the open buffer's text: its reads, the cursor's place, a scalar field, the tokens, the ▶ lines, a clip's window.
// A reader would get wrong that a span is the TREE's: map it through the edits made since the text was parsed before it is used (ui-editor's tree.ts).

import { call } from "./abi.ts";
import type { BufferReads, ClipGrid } from "./types/index.ts";

export * from "./ops/buffer.ts";

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
