# The interactions Sheet

The [K7 DAW Interactions Sheet](https://claude.ai/artifact/GWymnBJpibPCE4b32ZR1gz) is the single source of truth
for k7's screens and their interactions, on every device. [INTERACTIONS.md](INTERACTIONS.md) and the CSVs are
generated from it, one way only: neither is edited by hand, and nothing flows back into the Sheet.

## What the Sheet holds

- **Interactions**: one row per interaction — its ID, its screen, the mouse / trackpad, keyboard and touch ways,
  the devices (`T` iPad, `M` iPhone, `D` Desktop), whether the element is drawn on the screen's boards, its area.
- **Screens**: one row per screen, with the names of its boards on the [design canvas](https://claude.ai/artifact/SdfxjgJHEPVHgzTiimzYY6) and whether it is drawn. The `Interactions board` and `Canvas y` columns are unused.
- **Areas**: the one-line description that opens each area the first time it appears.
- **Gaps**: what was found missing or inconsistent while listing.

## Choices

- **An ID is permanent.** `03.05` is screen 03's fifth interaction when it was listed; it is never renumbered or
  reused. A row that moves to another screen keeps its ID, and a new row takes the next free number of its screen.
  Designs, tests and reviews name interactions by ID, so a renumbering would silently repoint them.
- **The CSVs are committed** ([interactions/](interactions/)), so a refresh shows as a reviewable diff of rows
  before the generated files move.
- **A branch that changes an interaction updates the Sheet**, then refreshes; it never edits INTERACTIONS.md or the
  CSVs by hand, since the next refresh would erase that edit.
- **The canvas holds the screens' boards only.** Interactions live in the Sheet and nothing draws them on the
  canvas: the canvas is reorganised into pages by hand, and generated interaction boards fought that layout.

## Refreshing

1. Export the Sheet's four sheets as CSV into `apps/k7-daw-frontend/docs/interactions/` (`interactions.csv`,
   `screens.csv`, `areas.csv`, `gaps.csv`): the Sheet's page downloads each sheet as CSV, and an agent with the
   Claude Docs connector reads the cells (`readCells`) and writes the same files. The connector has no export
   call for a Sheet.
2. `node scripts/interactions/build.mjs` writes INTERACTIONS.md.
3. Commit the CSVs with INTERACTIONS.md.
