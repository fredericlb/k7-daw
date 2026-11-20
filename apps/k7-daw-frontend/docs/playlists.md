# Playlists — a set list and the files it names

The format, and why a playlist lives in the workspace, are in
[`apps/k7-daw-frontend/packages/song/docs/playlists.md`](../packages/song/docs/playlists.md). This page is the
screen.

```
                                        ┌─ panel, on the RIGHT ─┐
cog ▸ Playlists   (or the set's button) │ ▾ été 2026      + ⤢ ✕ │  ← searchable
                                        ├───────────────────────┤     dropdown
                                        │ the records,          │
                                        │ resolved and playable │
                                        └───────────────────────┘
        ⤢ ──────────────────────────────▸ the PAGE: k7x text | the records
```

## A panel anchored right, not a page

A set list is opened **while** you are working, and a full-screen page forces you to leave the song
to look at it. So it is a panel **stuck to the right**, in the same row as the panes: it pushes them
rather than covering them, and it can be left open.

The playlist is chosen in a **searchable dropdown** — the `TreePicker`, the module picker's. Same
reason as everywhere else: the list is as long as the workspace is old, and typing three letters is
this shell's gesture throughout. The search scores the title AND the path, because a set list is
looked for by either (`live/repet`).

The records are **below**, resolved against the workspace's audio files and playable — that is
`PlaylistPane`. A row shows the file's name and, beside it, the folder holding it.

## The text stays, one click away

The panel is 24rem: there is no room for the k7x beside the records. So the full-screen page is one
notch further out, and the ⤢ button opens it on the current playlist.

Every line carries its three buttons — ↑, ↓, × — because reordering is what this screen is ABOUT,
usually shortly before a gig, and having to open the k7x to remove a line is the wrong shape for
it. The page stays what it is: the whole file, its comments, its title.

Those buttons are not writes: they go up to `PlaylistEditor` exactly as the search does, and it is
the BUFFER that changes. A panel writing the file while the text half holds an unsaved version of it
would make two truths of one document. The verbs `playlist song remove` and `playlist song move` do
write the file: they serve the CLI and the command bar, where no buffer is open
(`apps/k7-daw-frontend/packages/commands/docs/playlist.md`).

A shape the text cannot reorder — an inline `items: [a, b]` — disables the three buttons with the
reason in a tooltip, rather than reformatting somebody's file.

It is the same component on both sides. `PlaylistEditor` takes a `panel` prop, which removes its
header and the k7x half; below that everything is identical — same load, same buffer, same
autosave, same resolved list. Two components would be two documents to keep in step.

## Its own panel, not a settings tab, not a pane

This shell's panes are the **open song** — editor, timeline, graph, viewport — and what they draw is
what the project DECLARES (`docs/shell.md`, `apps/k7-daw-frontend/packages/song/docs/surfaces.md`). A playlist is
declared by no project and belongs to none: it is at the root of the workspace and it is the same
whichever song you are working on. So it is not a pane, even now that it has a column — and it is
not in the URL either.

Nor is it a **settings tab**, though the cog is what opens it: that would file it by where you
arrive from. Settings are what you set and leave; a set list is what you work FROM. So it shares the
right-hand column with settings and sharing (`rightPanel` in `App`, `docs/shell.md`) without being
filed inside them.

Like everything else in this shell, what opens it **dispatches a command line** — `ui playlists`,
which toggles the panel — rather than calling a function of its own: everything on this screen is
reachable from the palette, and a single implementation is what stops the two diverging
(`docs/shell.md`).

## A button in the bar, on a SET only

A project declaring a `turntables` surface carries a **Playlists** button in the bar, beside Share
and Settings. On a desk the playlist IS the set list: it is the screen you open mid-set, and hunting
for it in the palette is the wrong shape for the one thing you look for during the evening.

On a song there is no button — the bar is only so wide, and set lists are not what you came for.
`ui playlists` still opens them: same line, so one behaviour and one journal entry.

## One server, the account's file service

Everything this screen reads and writes is the workspace's, so it talks to the file service and to
nothing else: the playlist itself — `/playlists`, `/file`, `PUT /file` — and the audio a row plays,
through `mediaUrl`. Authentication is the session cookie, which is also what makes that URL work in
an `<audio src>`: a media element cannot set a header, and does not have to when the request is
same-origin.

`/auth/*` at the top of `api.ts` stays bare: k7 is the service whose origin holds the IdP's one
`redirect_uri`.

## One new route

`GET /playlists` lists; **reading and writing a file are `/file` and `PUT /file`**, which already
serve any path in the workspace. A second pair spelling `_playlists` into its URLs would be two ways
to write one file — and the autosave this screen runs on is built on the one that exists.

The listing is a separate route for the reason `/module-libraries` is: `/tree` is rooted on the
active project, deliberately, so the folder is invisible to it.

There is **no** `POST /playlists` either, by the same reasoning one notch further: creating a
playlist is a **command**, so the write goes through `/command` with everything else. Its rules —
the template rather than an empty file, the extension added, an existing playlist refused — are the
verb's, with its tests.

## Creating and adding are verbs

| what you do | the command |
|---|---|
| "New playlist" | `playlist new <nom>` — subfolders included (`live/repet`) |
| add a record, by path | `playlist song add <playlist> <record>` |

Both act on the **workspace** (`repoDir`) and not on the project: they are the only verbs in the
catalogue that work with no project open, which is exactly what a playlist is. The record is **all the
rest of the line**, spaces included, because a reference is a path through the workspace's folders
(`sets/AC-DC/Hells Bells.mp3`).

**Adding by search, in the right-hand pane, does not dispatch the second**, deliberately: a click
writes into the **open buffer** beside it, for the reason above. The two paths share the one thing
that must be shared, `appendPlaylistItem` — the same relation `event trim`'s gate preset has with
`trimWindow` (`docs/shell.md`): one implementation, two callers, at two different levels.

So the verb is what you use **from the palette, a key or the CLI**, when the screen is not open —
and that is where it earns its keep: building a setlist without leaving the timeline.

## The right-hand pane follows keystrokes, not saves

`items` is recomputed on every keystroke, but is **only replaced by something readable**: half-typed
k7x fails to parse about every other keystroke — a bracket not yet closed — and a panel that
emptied each time would flicker for the whole length of the line being written. The footer says when
what is shown is behind the text.

There is no Save button: the file is written as soon as the buffer **reads as a playlist**. A
half-typed line elsewhere in the file therefore holds the addition back too — that is the point, the
file is written whole or not at all. See
[autosave.md](../packages/ui-editor/docs/autosave.md).

## Where a line meets the files

`parsePlaylist` keeps its references verbatim, because resolving them needs the workspace's file
list. This screen is where a line and the files are both in hand, and therefore where "that record
does not exist" is a question that can be answered.

The resolution itself is pure and tested separately (`resolveRecords`, in
`@studio/song-schema/records`): a line is a workspace-relative path, nothing else. The desk reads
the same function, so a row greyed here is a row greyed there.

A line resolving to nothing **stays on screen**, and that is the point: the file is edited by hand,
just to the left. A line that vanished silently would be a typo you cannot see. A row saying "not
in this workspace" is a typo you fix. A line leaving the workspace, or naming a file that is no
audio, says so in its own words (`not a workspace file`).

## Two buttons

**"Search"** opens a field above the list — above and not over it: what you are filling stays
visible, and watching it grow is the only confirmation there is. The search runs over the
workspace's audio files by path, so the folder counts, and **every word must land somewhere, not
in the same place**: "nirvana lithium" finds a record where one is the folder and the other the
file. Enter takes the first result, Escape closes. The field **stays open** after a click:
building a setlist means adding eight records in a row. The same list feeds the k7x completion
(`playlistVocabulary`), so the search and the editor agree on what a line may name.

A record arrives in the workspace the way any file does, in the file tree; this screen only
*names* it, and there is no second door for depositing one.

### "Stems" — preparing the whole set list

The second button (with **Start (n)** beside it in the header), and the only one that costs hours. Each record is split into drums / bass /
other / vocals into the `stems/` folder beside it, so that one can be **removed** on the desk
during the set
([../packages/k7-plugins/docs/turntables/stems.md](../packages/k7-plugins/docs/turntables/stems.md)).

It is here and not on the desk because it is a question of **set list** and not of record: the
desk never separates, this screen prepares the twelve you are going to drop. The button opens a
bar above the list (the chain and the progress; Start stays in the header) — same place as the search, and for the same reason.

- **The chain is not asked for here.** It is DISPLAYED — the bar shows the one that will run and no
  more. Which module separates and which model it wants is a single answer for the machine: given
  once in k7's global settings, *Separation chains*, living in `<state>/preferences.k7x`
  (`system stems`) — see [settings.md](settings.md). A picker here would ask the same person the
  same question on every set list, and three screens that ask it are three places to get it wrong.
  The preference is re-read every time the bar opens, because settings are one click away.
- **It dispatches `playlist stems <playlist>`.** The verb is the work, and it is what you use from
  the palette or the CLI; the bar is only the place to start it and read how far it got.
- **One record at a time, and relaunching costs nothing.** Two separations in parallel on one
  machine are two hours of arithmetic fighting over the same cores and finishing no sooner, and a
  record whose `stems/` folder is complete is skipped.

What is already separated shows on the row: a record whose four stems are beside it carries the
**separation icon** on the right. It is read off the workspace's file list the pane already holds,
so not one more call per line, and it answers "which are left to prepare?" before launching.

The **Start** button is dark while no chain is set, with the sentence saying where to go and a
shortcut into settings — finding that out at the first record would be finding it out too late.

## The transport is ours, and it is tiny

One `<audio>`, one line at a time, on a same-origin URL.

It **starts on a row** and nowhere else: there is no "Play all" in the bar, because reading a
setlist from the top is what the desk does, with the engine behind it. What you listen to here is
the line you have just added — and the next one follows on, which gives "from here" without a
button having to say so. This shell is an editor that already has a transport of its own, driving
the engine on a project: a second player outliving the screen would be two things claiming the
word "playing".

An unplayable line is **skipped**, not blocking: a playlist that stopped at its third line over a
typo would be a playlist you cannot listen to while you fix it. The row says what is wrong; it does
not stop the evening.

A row is also **draggable** onto a deck of an open `turntables` surface: what travels is the
record's path, under our own MIME type (`apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/setList.md`).

## What a playlist is for, once written

It is not only a list to re-read: it is a **surface's parameter**. A `turntables` surface — the DJ
desk — carries a `playlist` line, and that is the playlist its record browser lists:

```k7x
// <projet>.k7/surfaces/desk.k7x
type("turntables")
layout("split")
playlist("ete-2026")
```

Two consequences, and the second is the one that matters:

- the desk searches the evening's twelve records rather than every audio file of the workspace;
- the records the playlist names are **copied to the machine that plays**, with the project and
  ahead of the set, because the engine opens file paths and the venue has no wifi.

That is also what gives this screen its stakes: the playlist filled in here is the one that gets
played. See `../packages/k7-plugins/docs/turntables/setList.md`.
