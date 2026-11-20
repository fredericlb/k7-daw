# Tokens and the kit

`src/theme.css` is the one place the look is decided: the `--k7-*` tokens read from the design
canvas, the `--sh-*` keys Tailwind's `bg-*` and `text-*` classes resolve through, and one `@utility`
per surface and per type step. An app imports it once and owns no colour of its own beyond the
meaning colours in its `index.css`.

## Choices

- **Dark only.** The canvas draws no light theme and the accent is fixed, so the tokens sit
  straight on `:root` and there is no setting.
- **Glass has no blur.** Only `float` and `panel` surfaces blur. A reader who adds a blur to
  `glass` makes every header control cost a compositing layer for nothing.
- **A control is a pill**: its radius is half its height, and its minimum target is 44 px on
  every device, desktop included. Buttons are 44 tall, or 36 inside a card, a toast or an editor.
- **The accent is a fill, never text.** Accent-coloured text and icons use `--k7-accent-text`
  (`text-primary-text`); pressed and selected states use the lavender `--k7-on` (`text-lavender`).
- **Two faces, three weights.** Sansation (300, 400, 700) sets every word; JetBrains Mono (400
  and 600) sets numbers, rulers, timecodes, commands and code. Both are bundled with Fontsource, so
  the page never waits on another host. There is no display face: `--font-brand` is Sansation.
- **An armed lane uses the record red** `#ff5a4e` (`--k7-record`). Arm-on, the record dot and the
  playhead while recording are one token, so a take being recorded and the lane that is armed for
  it read as the same thing.
- **Surfaces are utilities**, not components: `glass`, `float`, `panel`, `panel-dialog`,
  `panel-sheet`, `menu`, `row-group`, `scrim`. `Card` takes a `variant` over the same names.
- **The `--sh-*` keys stay**, pointing at the canvas, so code that has not been restyled keeps
  compiling. `--color-secondary`, `muted` and `accent` are one translucent row fill; a hover is
  `on-fill`.
- **The tab lozenge slides.** A segmented control (`Tabs`, `Choice`) and the home `TabBar` draw
  one pill under the selected label, positioned from the label's box, instead of painting each
  tab. Under jsdom there is no box, so no lozenge draws.
- **Motion is off under `prefers-reduced-motion`**, for every animation the tokens define.

## The new components

Each has its own `exports` subpath and a story that shows every state `components.md` lists.

| subpath | what it is |
|---|---|
| `capsule` | `Capsule`, `CapsuleButton` (pressed state), `CapsuleDivider` |
| `row-group` | `RowGroup`, `Row`, `RowButton`: icon, label, detail, trailing, disclosure, selected, tall |
| `side-panel` | `SidePanel` (400 px, floats over the panes) and `Sheet` (phone, with a transport slot) |
| `editor-dock` | `EditorDock` (the bottom editor dock of Pad and Desk, resizable) and `EditorSheet` (phone), each with the Expand toggle |
| `tab-bar` | `TabBar`, `TabBarItem` (a link with `href`, else a button), `TabBarRound` |
| `spinner`, `progress` | the running glyph and the 6 px determinate bar |
| `chip` | `Chip` with `variant` anchor (orphan), version (newest), filter, status (`tone`) |
| `empty-state`, `error-card` | the dashed empty card and the stay-until-fixed failure |
| `choice` | a radiogroup: segmented, or 44 px rows on a phone |
| `picker` | a 56 px button and a searchable list under it |
| `knob` | the 270 degree arc knob with a peak chip |
| `track-toggle` | M, S and A: one component for the lane header and the mixer strip |
| `backdrop` | the two blobs and the two wave strokes behind a screen |

A page decides the device layout (`Choice layout`, `SidePanel` against `Sheet`); the kit never
reads the viewport, because the shell already knows whether it is folded.
