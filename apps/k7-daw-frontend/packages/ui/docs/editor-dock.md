# The editor dock

`@studio/ui/editor-dock`: `EditorDock` (Pad and Desk) and `EditorSheet` (Phone). What reads
left to right sits at the bottom: the editors, the Mixer and the per-track chain, one occupant at a
time; the right panel (`side-panel`) is for the vertical lists.

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
- **The header carries the open editor's first row, not a title.** The row (clip name, counts, tools)
  sits on one line with Expand and Close, so the chrome is one line and the dock's height goes to
  the editor. The editor keeps its state and fills the header through `DockHeaderRow` (a portal into
  the dock's slot); the title stays as a visually hidden name, so the dock is still found by it. The
  empty dock has Expand and Close only.
- **The dock holds one occupant and is named by it.** The host swaps the children, the title and
  the Dialog name together; the header row is the occupant's own (`DockHeaderRow`), so a Mixer
  shows its count and a chain its track where an editor shows its tools. Strips and cards scroll
  sideways inside the body, and on a phone Expand takes the sheet full screen.
- **Escape leaves the expanded state before it closes the dock.** The kit does it for its own Escape;
  a host registering the dock in an Escape ladder must do the same in its closer.
- **The height is the host's.** `EditorDock` is controlled (`height`, `onHeightChange`) so the host can
  place what sits beside it (a right panel ends `--k7-dock-gap` above the dock, toasts too). The
  grabber clamps the drag between `--k7-dock-min-h` and the expanded height, and is a button, so the
  arrow keys (Up and Down by 16 px, Home and End for the extremes) resize it as well.
- **The phone sheet is partial and non-modal** (`--k7-dock-sheet-top`) so the lanes stay in view;
  expanded it takes `--k7-sheet-top` and traps focus like any sheet.
