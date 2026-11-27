// notes.ts — the note grid's two types: a note as the grids hold it, and the rack they resolve symbols through.
// Reading and writing a sequence's `notes` list is Rust's (`readSequenceNotes`, `writeSequenceNotes` in @studio/k7-wasm/text). See docs/noteGrids.md.

import type { GridNote } from "@studio/k7-wasm/text";

export type Note = GridNote;

// A resolved rack: the symbol<->note maps the grids use, defined in this leaf module so neither side depends on the other.
export interface RackInfo {
  toSymbol: Map<number, string>; // MIDI note -> symbol (for display)
  toNote: Map<string, number>; // symbol -> MIDI note (for entry)
  order: { symbol: string; note: number }[]; // ascending by note — a stable palette
}
