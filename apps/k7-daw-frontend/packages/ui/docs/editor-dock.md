# The editor dock

`@studio/ui/editor-dock`: `EditorDock` (Pad and Desk) and `EditorSheet` (Phone). The editors are
horizontal and sit at the bottom; the right panel (`side-panel`) is for everything else, the
per-track chain included.

## Choices

- **It floats over the lanes and pushes nothing**, like the right panel: a pane is laid out once.
  It is `absolute`, left and right 12, bottom `--k7-panel-bottom`, so the floating bar stays under it.
- **Non-modal on Pad and Desk.** People keep working in the lanes while an editor is open, so an
  outside interaction is ignored and focus is not taken. It keeps `role="complementary"`, named by
  its title, like a right panel: a suite finds both the same way.
- **The Expand toggle is a toggle, not two buttons.** Its name is the same in both states and
  `aria-pressed` says which one it is in; the glyph (`Maximize2` / `Minimize2`) and the lavender
  "on" look follow it. Expanded on Pad and Desk the top goes to just under the header and the bottom
  stays above the bar; on a phone the sheet goes full screen and becomes modal.
- **Escape leaves the expanded state before it closes the dock.** The kit does it for its own Escape;
  a host registering the dock in an Escape ladder must do the same in its closer.
- **The height is the host's.** `EditorDock` is controlled (`height`, `onHeightChange`) so the host can
  place what sits beside it (a right panel ends `--k7-dock-gap` above the dock, toasts too). The
  grabber clamps the drag between `--k7-dock-min-h` and the expanded height, and is a button, so the
  arrow keys (Up and Down by 16 px, Home and End for the extremes) resize it as well.
- **The phone sheet is partial and non-modal** (`--k7-dock-sheet-top`) so the lanes stay in view;
  expanded it takes `--k7-sheet-top` and traps focus like any sheet.
- **The dock lays nothing out inside.** Its body is a plain scroll box; the editors choose their own
  columns from a container query on it, so the kit stays free of any editor's shape.
