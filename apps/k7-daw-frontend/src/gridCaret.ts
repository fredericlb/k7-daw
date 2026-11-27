// gridCaret.ts — what the shell drives on whichever clip grid is on screen, and the grid's own cell types.
// The caret's and the rectangle's arithmetic is apps/k7-daw-wasm/gesture/src/grid_caret.rs, reached through @studio/k7-wasm/gesture.

import type { Clipboard, MaybeClipboard } from "@studio/k7-wasm/clipboard";

export type {
  GridBounds,
  GridBox,
  GridCell,
  GridSelection,
} from "@studio/k7-wasm/gesture";

/** What the shell drives on whichever clip grid is on screen — a handle rather than props, because these are COMMANDS. */
export interface ClipGridHandle {
  /** `v` — caret ⇄ rectangle. */
  toggleSelect: () => void;
  /** `t` — send the transport to the caret. */
  seekToCaret: () => void;
  /** Shift+L — the rectangle becomes the loop region, or clears it when there is none. */
  toggleLoop: () => void;
  /** `selection copy` — the rectangle's contents, or null when it holds nothing. */
  copy: () => MaybeClipboard;
  /** `selection paste` — the command LINE that would put the clipboard down at the caret, so one paste is one Ctrl+Z. */
  pasteLine: (clip: Clipboard) => PasteResult;
  /** `selection delete` — the LINE that removes what the caret or the rectangle points at; absent on a grid that deletes by clicking. */
  deleteLine?: () => PasteResult;
}

/** What a pane answers `selection paste` with: what to run, if anything, and what to say. */
export interface PasteResult {
  line?: string;
  note: string;
}

/** Whether a rectangle is live, reported UP rather than asked for: a condition is computed in a memo, which cannot read a ref. */
export type OnGridSelection = (live: boolean) => void;
