# The note grid — RollGrid, and the plugin that mounts it

A piano roll over a sequence file's `notes([ … ])` list. It is plain DOM, outside the React
tree, and it calls back with the WHOLE new note list, which `notes.ts` writes back into the
file.

## `plugin.ts` — the mounting mechanism

**`mountSeqGrid(container, opts)`** renders the roll into a plain filling element, with no
CodeMirror anywhere near it. It takes the file's content and hands back the WHOLE next
content with the note list rewritten, and re-rendering is driven from OUTSIDE it
(`controller.update`) — so an edit and an external reformat share one path, with no
re-entrant rebuild in the middle of an event. This is what every host uses.

There is no in-place mounting — a grid replacing the `notes` list inside the text editor,
the text around it still readable. A host wanting that property builds it on `mountSeqGrid`.

## Where a host puts them

This package renders a grid; it decides nothing about where. In k7 there are three homes,
and the split is deliberate:

| where | what persists an edit |
|---|---|
| the **chains slot** in the main view, opened by `sequence edit roll <sequence>` and closed by its × | written straight to the file, per edit (`SeqGridFile`) |
| the **editor popin's tabs** — Text / Roll, on a sequence | into the editor's buffer: dirty flag, Save button, one path with the text |
| anywhere else drawing a sequence | its own |

`SeqGridFile` is the file behind the chains slot: "read it, hand it to the grid, write every edit
back" is one behaviour, held apart from the frame.

It re-reads when the project moves — an undo, a command, a MIDI pass rewriting the same sequence —
because a grid that kept its first read would write that stale snapshot back over them on the next
cell edit. Never under a hand, though: not while a write is in flight, and not within a second of
an edit. And its writes go one at a time, since two whole-file writes in flight can land the older
one last. The clip pane's piano roll is a different roll — React, over the document, not the
file: see `apps/k7-daw-frontend/docs/clipEditors.md`.

The fullscreen editor pane has neither, on purpose: it is the text editor, and a grid
wants the room the main view has. See `apps/k7-daw-frontend/docs/shell.md`.

## `RollGrid.ts` — the piano roll

Mounted into a standalone pane by `plugin.ts`'s `mountSeqGrid`, filling whatever it is
given. Time runs left→right, pitch top→bottom, one lane per semitone. A left gutter names each
pitch (the piano keys); a top ruler numbers the bars.

The note area FILLS the pane — so gridlines and notes are positioned in PERCENTAGES
of it, so it stretches both ways with the pane instead of scrolling a fixed grid. Click
an empty cell to add a one-step note; click a note to remove it.

## `notes.ts` — the read/write seam

A sequence holds its notes as a flat list of chains, one per line —
`t(0).d(1).k(60).v(0.8)` (t/d in beats, k a MIDI key, v an optional 0..1 velocity) — or in
the short form `notes("c4 d4 ~ e4")`. The roll goes through here, so a grid edit round-trips
to the exact text the compiler reads back.

`readSequence` reads the notes EXPANDED, so a short form shows on the grid like any list.
`writeNotes` hands the grid's list to `@studio/k7-wasm/text`'s `writeSequenceNotes` and makes the
file's data equal it with the fewest edits (`applyIn`): one note moved is one line changed,
and the comments, the `use_rack` and the window around the list are never rewritten. A short
form stays while it still expands to the grid's notes, and is written out in full the moment
it does not — writing it back would undo the edit.

## The window the roll draws

A sequence's `from`/`to` say which span of its notes actually PLAYS. The roll lights that
span as ground, as a band behind the lanes, with its two edges picked out.

Three things about how it gets there:

- **It is read from the whole document, not from the notes array**, like `use_rack`: the
  pair sits outside the list. `seqWindowFromText` in
  `@studio/k7-wasm/rules` is the reader, called on every render, since editing `from`
  changes neither the notes nor the rack.
- **The window joins the content span.** A `to` reaching past the last hit is a pad, and a
  grid that stopped at the notes would clip the very edge that explains the silence after
  them.
- **At 4/4, like everything else in these files.** Its bar lines and its ruler
  assume it already; reading the meter here alone would put the
  band on a grid drawn to another one.

## A pad's effects survive a grid edit

`notes.ts`'s own `Note` is `{ t, d, k, v? }`. A sequence's schema has two fields more —
`repeat` and `skip`, a pad's effects (`apps/k7-daw-wasm/song/docs/padEffects.md`) — and since these
grids call back with the WHOLE list, a write that took the list as given would strip them from
every pad on the first click.

So `writeNotes` carries them: each note the grid hands back takes the effects of the note that
started on the same key at the same beat. A note the grid MOVED loses them: the grid hands back
a list and no identity, and key and onset are the only address a note has in it.
