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
- **A control is a pill**: its radius is half its height. On a touch screen its minimum target is
  44 px: buttons are 44 tall, or 36 inside a card, a toast or an editor.
- **A desk is denser**: under a mouse, in a window that is not phone-sized, buttons, capsules, inputs,
  menu rows and the sidebar's rows are 32 px (a capsule's segments 24, the git capsule's two 30) and a
  control's 20 px icon is 18. The `desk:` variant carries it,
  and its media query is `useShellLayout`'s Desk, so a touchscreen or a narrow window keeps 44. A mouse
  needs no 44 px target, and the Desk boards draw every control 32 tall.
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
- **Motion is off under `prefers-reduced-motion`**, for every animation the tokens define. A state
  that only a motion showed (a lane recording) keeps a static ring beside it.
- **A control's edge reaches 3:1** against its ground (WCAG 1.4.11): the input line, the switch's
  off track and an off M/S/A button's ring are 0.4 white, since 0.1 to 0.24 read as no edge at all.
- **Type is counted in `--k7-u`**, 1px by default. `textScale.ts` scales it from the system text size
  on a touch screen of an Apple device, since WKWebView does not apply Dynamic Type to px; the layout
  stays in px, so a larger text grows inside the same boxes.
- **An overlay keeps the keyboard** (`modal-focus.ts`): Tab stays inside a modal one, and closing gives
  focus back to the opener unless a command already moved it. The kit's dialog names its Close button
  through `DialogCloseLabel`, which the app provides in its language.

## The new components

Each has its own `exports` subpath and a story that shows every state `components.md` lists.

| subpath | what it is |
|---|---|
| `capsule` | `Capsule`, `CapsuleButton` (pressed state), `CapsuleDivider` |
| `row-group` | `RowGroup`, `Row`, `RowButton`: icon, label, detail, trailing, disclosure, selected, tall |
| `side-panel` | `SidePanel` (400 px, floats over the panes) and `Sheet` (phone, with a transport slot) |
| `editor-dock` | `EditorDock` (the bottom editor dock of Pad and Desk, resizable) and `EditorSheet` (phone), each with the Expand toggle |
| `tab-bar` | `TabBar` (a phone's bar), `TabSidebar` (the sidebar elsewhere), `TabBarItem` (a link with `href`, else a button; `row` in the sidebar), `TabBarRound` |
| `spinner`, `progress` | `Spinner` (`size` xs, sm, md; `inherit` takes the text colour inside a button), `Loading` (a spinner and the caller's label) and the 6 px determinate bar |
| `chip` | `Chip` with `variant` anchor (orphan), version (newest), filter, status (`tone`) |
| `empty-state`, `error-card` | the dashed empty card and the stay-until-fixed failure |
| `choice` | a radiogroup or tablist: segmented (words or glyphs), or 44 px rows on a phone |
| `picker` | a 56 px button and a searchable list under it |
| `slider` | a native range with the one commit rule: `onValueCommit` fires on pointer up or cancel, key up or blur, once per change |
| `knob` | the 270 degree arc knob with a peak chip; `onValueCommit` fires on release, `defaultValue` is what a double-click restores |
| `track-toggle` | M, S and A: one component for the lane header and the mixer strip |
| `backdrop` | the two blobs and the two wave strokes behind a screen |

**Danger is `error`**: `text-error`, `bg-error`, `border-error`; there is no second name for it.

A page decides the device layout (`Choice layout`, `SidePanel` against `Sheet`); the kit never
reads the viewport, because the shell already knows whether it is folded.
