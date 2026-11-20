# Toasts

`src/toast.tsx` is the toast every studio app shows: `<Toaster/>` mounted once at the root,
`showToast()` called from anywhere. `showToast` answers the toast's id, which `updateToast` and
`dismissToast` take.

## Kinds

- **success**, **warning**, **error**: a warning leaves on its own after a longer delay than a
  success; an error stays until it is dismissed, since one nobody read did not happen. Warning and
  error are `role="alert"`.
- **neutral**: no judgement, with an optional `progress` (a 6 px bar, and `34 / 120` in mono when
  there is a total). A neutral toast with progress belongs to the code that runs the work: it has
  no Dismiss and stays until `dismissToast`.
- An **action** is a 36 px pill after the text (Restart). An **icon** replaces the kind's glyph.

## Placement

- **bottom-right** sits above the floating bar, clear of the right panel's bottom edge
  (`--toast-bottom`), and moves left of an open panel through `--toast-right` (428 px).
- **top-right** is for what must not be missed behind the bar.
- **top-centre** is the plugin-scan toast: above every other layer, sliding down from above, and
  leaving by itself when the code dismisses it.

On a phone the stacks run from left 16 to right 16. Bottom toasts sit 140 px above the edge, over
the bar; only the top-centre toast goes to the top (112 px down). A folded shell never moves the
bottom stack to the top: a toast at the top covers the header's controls, and the bar stays reachable
under a bottom one. `<Toaster phone>` takes the shell's own answer; without it the stacks follow the
viewport width.
