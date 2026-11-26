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
- **Help and status are title-bar pieces too.** `DockHint` is one info glyph whose tooltip and name
  are the help; `DockStatus` is the status line an editor shows with nothing to edit (loading, empty,
  an error), the body left blank. Both go through `DockHeaderRow`, so outside a dock they render inline.
- **A setting reads as "name: value".** `DockProperty` (`dock-property.tsx`) shows one of an editor's
  settings as text in the title bar ("gain: 0.0 dB", "loop: off"); a tap opens a bubble above it
  holding the control that changes it, a knob or a switch. Every editor shows its settings this one
  way, so the title bars read alike whatever the control behind them.
- **The header row wraps.** The slot is a wrapping flex row, so an editor with many options takes a
  second line instead of pushing them into its body (`apps/k7-daw-frontend/docs/editors.md`).
- **A row may stay in the body on a phone.** `DockHeaderRow bodyOnPhone` draws its row in the header on Pad and Desk and in the sheet's body on a phone, where the header is a name over its counts (`useInSheet` tells an editor which frame it is in).
- **The dock holds one occupant and is named by it.** The host swaps the children, the title and
  the Dialog name together; the header row is the occupant's own (`DockHeaderRow`), so a Mixer
  shows its count and a chain its track where an editor shows its tools. Strips and cards scroll
  sideways inside the body, and on a phone an expanded sheet is full screen.
- **Escape leaves the expanded state before it closes the dock.** The kit does it for its own Escape;
  a host registering the dock in an Escape ladder must do the same in its closer.
- **The height is the host's.** `EditorDock` is controlled (`height`, `onHeightChange`) so the host can
  place what sits beside it (a right panel ends `--k7-dock-gap` above the dock, toasts too). The
  grabber clamps the drag between `--k7-dock-min-h` and the expanded height, and is a focusable
  `separator` with `aria-valuenow`, so the arrow keys (Up and Down by 16 px, Home and End for the
  extremes) resize it as well. Its hit area is 44 px tall, reaching above the dock so the header's
  row stays clear; the pill is 5 px.
- **Expanding is a gesture on the grabber, not a button.** A press held still for 400 ms, then moved
  24 px up, expands the dock; moved down, it folds it back. The pill widens and brightens once the
  press is held, so the person knows the next move expands instead of resizing; a press that moves
  at once is a resize. Enter on the focused grabber toggles it, the keyboard's way to the same thing.
  The title bar keeps the editor's own options and Close only.
- **The phone sheet has the same grabber**, expand and fold only (nothing resizes a sheet); its name
  is `expandLabel`, `aria-pressed` says the state, and Enter or Space toggles it.
- **The phone sheet is partial and non-modal** (`--k7-dock-sheet-top`) so the lanes stay in view;
  expanded it takes `--k7-sheet-top` and traps focus like any sheet.
