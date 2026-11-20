# The Mix view reads a playlist

The record list comes from the **playlist the surface names** — a `playlist:` line in
`surfaces/<desk>.k7x` — and not from every audio file the workspace holds. That is what makes
this view speak about the open project like every other one, and why the project tree stays
beside it.

## It is a PARAMETER of the surface

```js
// surfaces/desk.k7x
type("turntables")
layout("split")
playlist("ete-2026")      // → <espace>/_playlists/ete-2026.k7x
mapping({ … })
```

A **name** and not a path, for the reason a module ref is one: the folder is fixed, so spelling
it in each declaration would give a file the right to be wrong about a location nobody chooses.
`playlistPathOf` does the conversion and refuses anything that would leave the folder.

And a parameter of the SURFACE, not of the project: the screen with a record browser is the one
that must say what it rummages in. A `timeline` carrying the same line would have it ignored —
so the schema refuses it instead, being `.strict()`: a field accepted and inert sends whoever
wrote it looking elsewhere.

## The cue points are not in the playlist

A playlist says **which** records the set plays, and it is the right document for that: it has a
screen (k7's cog opens it) and it is shared between sets. What it has nowhere to put are *this*
set's **cue points**, eight per record. A cue belongs to the desk playing the record and not to a
list others read — two bands covering one title do not drop the needle in the same place, and a
playlist carrying one set's marks would hand them to the other. So the cues live in
**`mix.k7x`**, at the project root, under `cues:` ([cues.md](cues.md)).

## A record is a file, named by its path

A playlist line names an audio **file of the workspace** by its workspace-relative path, and the
desk reads nothing else about it: there is no catalogue to consult. A row shows the file's name
and, above it, the folder holding it. `resolveSet` crosses the playlist with the workspace's
files — the same `resolveRecords` k7's playlist pane uses (`@studio/song-schema/records`), and
that is the point: "which file does this line mean" is a rule, and two copies would diverge the
day one of them learnt a case.

A line naming no file stays a row with nothing to play, and a record's stems are the `stems/`
folder beside it, as a clip's ([stems.md](stems.md)).

## Three states when there is no list

| what is true | what the desk says |
|---|---|
| the surface names no playlist | "add a `playlist:` line in `surfaces/…`" |
| it names one that is not on disk | "the playlist this surface names cannot be found" |
| it names one that is empty | nothing: a set list being filled in |

Three sentences because the repairing gesture differs each time. An empty browser with no
explanation reads as a workspace without audio, which sends you looking in the wrong place.

## Where the list is read

Not in the middle of the desk: a record is chosen **at the record itself** — a click at the
centre of a vinyl opens the picker ([songBrowser.md](songBrowser.md)), which is already the only
way to load a deck. A column repeating what the record knows would take the width the decks
want. The crossing, the three states and their sentences are that picker's, reading the same
`SetList`.

## A line that cannot play stays on screen

A record the playlist names and the workspace lacks **stays in the list**, greyed, with its
download icon. Dropping it would make a set missing three records look identical to a set that
never had them, on the one screen where that difference matters most. A record the workspace
holds reaches the engine's machine with the project, the file service copying and prefetching
what a playlist names.

The reasons Play does not work are therefore said separately: not in the workspace, no engine on
this machine.

## A duplicate, though, is removed

A playlist may legitimately name the same record twice — k7's screen draws both rows, because
there it is a document being edited. Here it is a **browser**: two identical rows are two chances
to load the same record without knowing which one you got.

The identity is the record's path: two lines naming one file are one record.

## Dragging a line onto a deck

k7's **playlist panel** — the cog's, docked right — can stay open while a `turntables` surface
is on screen. Its rows are therefore `draggable`, and each deck is a **drop zone**, strip and
knobs included: aiming at the vinyl's label is a gesture for a settled mouse, not for a hand
crossing the screen.

**What travels is the LINE, not a resolved file.** `RecordDrag` carries the one field a playlist
line *is* — the record's path — and the desk resolves it against its own set list, exactly as it
resolved the others. Sending an already-resolved entry would let another half of the application
decide what goes on the deck; the line can only lie about itself. It is also what keeps
duplicates meaning here what they mean above: the path is the same identity on both sides.

The format is **our own MIME** (`application/x-studio-record`) rather than `text/plain`: a browser
will drop almost anything onto a page — a file, a URL, a piece of selected text — and a deck
accepting `text/plain` would load whatever the user last copied elsewhere. `parseRecordDrag`
further refuses anything that is not the expected shape: the content of a transfer is written by
the page it came from.

**A record this surface does not know is refused by name.** The panel shows the playlist being
edited, which is not necessarily the one the surface names — a frequent case, not a programming
error. The drop says so in the view's error card rather than doing nothing, because a gesture
that fails silently reads as a bug in drag-and-drop.

## What plays, once the line is chosen

The project's **compiled** document, into which `withDecks` places both decks' clips — so the
set's real chains, crossfade included. A `jsdsp` module is a graph baked into the document by the
compiler, which runs the module's source in a child process: the browser cannot produce one, and
`deckDoc` is only a stand-in of one track per deck until the compile arrives. See
[mix.md](mix.md) and
[apps/k7-daw-frontend/packages/song/docs/setProject.md](../../../song/docs/setProject.md).
