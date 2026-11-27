// sliceSymbols.ts — a slice-rack file's symbols as the EDITOR paints them: the colour each slice wears everywhere (the dock's windows, the names in the text)
// and the CodeMirror extension that paints those names. Where each symbol's numbers sit, and the write of a window, are Rust's (`scanSlices`, `editSliceWindow`).
// The docked source waveform is THE slicer, one picture for every symbol (apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md).

import {
  type EditorState,
  type Extension,
  RangeSetBuilder,
  StateField,
} from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";
import { scanSlices } from "@studio/k7-wasm/buffer";
import { editSliceWindow } from "@studio/k7-wasm/text";
import { SERIES_COLORS, seriesColor } from "@studio/ui/series-colors";
import { editTextBy } from "@studio/ui-editor/k7x/bufferEdit";
import { k7xTree, mapSpan, treeOf } from "@studio/ui-editor/k7x/tree";

/** One colour per slice, by INDEX in the symbols map's own order, so 'the third slice is teal' is true in every view of a rack. */
export const SLICE_COLORS = SERIES_COLORS;
export const sliceColor = (i: number): string => seriesColor(i);

/** A rack's slice windows as the dock draws them, coloured on the same basis `sliceSymbolColors` indexes on. */
export function rackSliceRanges(
  symbols: readonly { symbol: string; from?: number; to?: number }[],
): { symbol: string; from: number; to: number; color: string }[] {
  return symbols
    .filter((s) => s.from != null && s.to != null)
    .map((s, i) => ({
      symbol: s.symbol,
      from: s.from as number,
      to: s.to as number,
      color: sliceColor(i),
    }));
}

/** Set one symbol's `from`/`to` in the buffer's text, the two numbers and nothing else; null means write nowhere. */
export const setSliceWindow = (
  text: string,
  symbol: string,
  from: number,
  to: number,
): string | null =>
  editTextBy(text, (t) => editSliceWindow(t, symbol, from, to));

/** The symbol names painted in their slice's colour — the link between a line of text and a window on the waveform. */
export function sliceSymbolColors(): Extension {
  const build = (state: EditorState): DecorationSet => {
    const tree = treeOf(state);
    const text = state.doc.toString();
    if (!tree.root || !text.includes("rack_type")) return Decoration.none;
    const syms = scanSlices(tree.text);
    if (!syms) return Decoration.none;
    const b = new RangeSetBuilder<Decoration>();
    syms.forEach((s, i) => {
      const span = mapSpan(tree, s.keyRange[0], s.keyRange[1], text);
      if (!span) return;
      b.add(
        span.from,
        span.to,
        Decoration.mark({
          attributes: { style: `color:${sliceColor(i)};font-weight:600` },
        }),
      );
    });
    return b.finish();
  };
  return [
    k7xTree,
    StateField.define<DecorationSet>({
      create: build,
      update: (deco, tr) => (tr.docChanged ? build(tr.state) : deco),
      provide: (f) => EditorView.decorations.from(f),
    }),
  ];
}
