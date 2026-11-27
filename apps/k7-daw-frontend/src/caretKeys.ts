// caretKeys.ts — which keys walk a caret: hjkl and the arrows, as a step in rows and columns. Shared by
// the timeline and the clip editors' grids, which differ only in what a step moves.

/** The (row, column) step a key asks for, or null when it is not a caret key. */
export function caretKeyDelta(
  e: Pick<KeyboardEvent, "key">,
): { dRow: number; dCol: number } | null {
  switch (e.key) {
    case "ArrowLeft":
    case "h":
      return { dRow: 0, dCol: -1 };
    case "ArrowRight":
    case "l":
      return { dRow: 0, dCol: 1 };
    case "ArrowUp":
    case "k":
      return { dRow: -1, dCol: 0 };
    case "ArrowDown":
    case "j":
      return { dRow: 1, dCol: 0 };
    default:
      return null;
  }
}
