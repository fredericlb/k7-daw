import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { wasmReady } from "@studio/k7-wasm/boot";
import { parse } from "@studio/k7-wasm/k7x";
import { beforeAll, describe, expect, it } from "vitest";
import {
  SLICE_COLORS,
  setSliceWindow,
  sliceColor,
  sliceSymbolColors,
} from "./sliceSymbols";

// Where the symbols sit and what a drag rewrites are Rust's and pinned there; what stays here is the editor's colours and the buffer write's seam.

beforeAll(async () => {
  await wasmReady();
});

const SLICE = `// a break, sliced
rack({
  rack_type: "slice",
  source: "break.wav",
  symbols: {
    BD: { note: 36, from: 0, to: 0.5 },
    SN: { note: 38, from: 0.5, to: 1.25 },
  },
})
`;

describe("setSliceWindow — what the dock's drag commits", () => {
  it("rewrites the two numbers of one symbol in the buffer's text", () => {
    const out = setSliceWindow(SLICE, "SN", 0.5374, 1) ?? "";
    expect(out).toContain("SN: { note: 38, from: 0.5374, to: 1 },");
    expect(
      (parse(out).rack as { symbols: { BD: { to: number } } }).symbols.BD.to,
    ).toBe(0.5);
  });

  it("answers null when the buffer holds no such symbol — a stray drag writes nowhere", () => {
    expect(setSliceWindow(SLICE, "HH", 0, 1)).toBe(null);
    expect(setSliceWindow('extends("x")\n', "BD", 0, 1)).toBe(null);
  });
});

describe("the colours in the text", () => {
  /** The text under every painted mark, in order. */
  const painted = (state: EditorState) => {
    const out: string[] = [];
    for (const d of state.facet(EditorView.decorations))
      if (typeof d !== "function")
        d.between(0, state.doc.length, (from, to) => {
          out.push(state.doc.sliceString(from, to));
        });
    return out;
  };

  it("paint each symbol name, and keep them while a line is half-typed", () => {
    let s = EditorState.create({
      doc: SLICE,
      extensions: [sliceSymbolColors()],
    });
    expect(painted(s)).toEqual(["BD", "SN"]);
    // Two characters at the top: the text stops parsing, and the names stay lit where they now are.
    s = s.update({ changes: { from: 0, insert: "x(" } }).state;
    expect(painted(s)).toEqual(["BD", "SN"]);
  });
});

describe("the palette", () => {
  it("gives neighbours different colours and cycles past its end", () => {
    expect(sliceColor(0)).not.toBe(sliceColor(1));
    expect(sliceColor(SLICE_COLORS.length)).toBe(sliceColor(0));
  });
});
