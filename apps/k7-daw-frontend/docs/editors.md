# Editors: options in the title bar, the content is what is edited

Every editor in the bottom dock looks the same: the chain, the Mixer, the piano roll, the audio clip,
the live take, a sequence's grid, the takes.

## Choices

- **Every option is in the title bar.** Tools, toggles, sliders, menus, Stop,
  Discard and Done are drawn through `DockHeaderRow` (`@studio/ui/editor-dock`) on the line of
  Close. An editor has no toolbar of its own in its content.
- **The content holds only what is edited**: the cards, the strips, the notes, the wave, the takes'
  rows, and the edited data's own names and values (a lane's name on its card, a key's name, a ruler's
  numbers). No caption, no section heading, no help line.
- **A setting is "name: value".** An editor's settings read as text in its title bar through
  `DockProperty` ("gain: 0.0 dB", "loop: off"); the knob or switch that changes one opens in a bubble
  above it on a tap. The waveform editor is the first; the others follow it.
- **Help is one glyph.** An editor's help is `DockHint`: an info glyph in the title bar
  whose tooltip and accessible name are the words. An editor has at most one, and it follows the tool.
- **Nothing to edit is said in the title bar, the content left blank.** Loading, an error, an empty
  dock or a lane with no module are `DockStatus`: a short status line in the title bar (in the error
  tone for an error), with the explanation, when there is one, in a `DockHint` beside it. A blank
  content reads as "nothing here" without a second pattern for empty states.
- **A crowded title bar wraps.** The audio clip carries the most options; the header row wraps to a
  second line rather than sending any of them back into the content.
- **An option that needs room opens from the title bar.** The trim gate's threshold and its preview
  are a popover anchored on Trim, so the wave keeps the space and shows what the gate keeps.
- **A phone may keep a row in the body.** The sheet's header is slimmer, so a tool row asks
  `DockHeaderRow bodyOnPhone` and becomes the body's first row there; it is still the title bar's row
  on Pad and Desk.
- **Names stay accessible.** A control moved to the title bar keeps its `aria-label`, and a segment
  drawn as a glyph keeps its word as its name and tooltip, so a suite finds it by role and name.

## Outside the dock

The editors that open elsewhere (the curve editor's dialog, the module builder, the text editor, the
chains and timeline views, the picture window, a surface) have their own frames and do not use
`DockHeaderRow` yet; the rule above is the target they are brought to.
