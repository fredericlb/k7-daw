# mix.k7x — a set's CUE SHEET

```k7x
cues([ref("sets/Daft Punk/Around the world.mp3").markers([12.5, null, 48.25])])
```

The only file a set has that a song has no counterpart for, and the only place a project names
its **records** (the audio files of its workspace) rather than its modules.

## It is not `views/mix.k7x`

Same base, different file, and the two have nothing to do with each other. A `views/mix.k7x`
holds the desk's *bindings* — which module each fader moves — and a surface carries its
`mapping:` instead, so a scaffolded set has none. This one is at the **root** of the project and
holds where the needle drops.

The editor's router (`apps/k7-daw-wasm/song/src/files/route.rs`) tells them apart by folder, and the order of its two branches is what does it: the
`views/` test comes first, so a root `mix.k7x` falls through to this schema.

## It is not the set list either

Which records the set draws on is answered by the **playlist the desk's surface names** —
`playlist:` in `surfaces/<desk>.k7x`, a file in the workspace's `_playlists/`
([playlists.md](playlists.md)). A set carrying a `songs:` by hand is TOLD — the schema is
`.strict()`, so it is refused by name rather than read as an empty sheet with every cue silently
gone.

Of the two documents, the playlist is the better place for a list of records: it has a SCREEN
(k7's cog opens it), it can be shared between sets and reused next season, and it is the thing
somebody has already written by hand. What stays here is the half a playlist has nowhere to put:
**the cues**. A cue belongs to the desk that plays a record rather than to a list other sets read —
two bands covering the same track drop the needle in two different places, and a playlist
carrying one set's marks would hand them to the other. So an entry appears here when somebody
drops a cue, not when somebody adds a record.

Everything below describes this file, except the two sections about the CACHE, which describe
what the playlist does — kept here because the reasoning is the same one, and it is the reasoning
that is worth having.

## A ref is the record's workspace path, verbatim

A ref is the path the playlist line carries (`isRecordRef`): workspace-relative, POSIX, already in
its plain form. A ref that **is** the path needs nothing resolved to be compared, deduplicated,
sorted or fetched, and a ref that resolves to nothing is still a perfectly good name for the thing
to go and fetch.

Not an absolute path: `project.k7x` is committed, and an absolute path breaks on the next machine.
Not a title either — two records share one.

The refs are strings and so are **quoted**, as a key and as a value. `graphFile.md` has the same
note for module refs.

## Unordered

The list has no running order. The argument is about a SET LIST — which records you brought, not
the order you will play them in, the order being a live decision — and it holds for the playlist
too, whose lines are sorted for display
([setList.md](../../k7-plugins/docs/turntables/setList.md)).

Here it means something smaller: the order cues are written in is the order they were dropped in,
and nothing reads it.

Duplicates are dropped rather than refused. Two people merging their halves of a file should not
have to resolve that by hand.

It is a **sequence** and not a map, although `graph.k7x` uses a map for exactly this "named
once" property. That decision is the load-bearing one — see the last section: a bare ref and a
ref carrying markers are the same shape, so cues need no migration.

## What the SET LIST is for: the machine must already have the records

*(This section describes the playlist. It stays here because the reasoning was worked out for
this file.)*

The machine that plays the set opens **file paths** — the engine cannot fetch a URL. So every
record in the set has to be on that machine's disk before you need it, and the venue has no wifi.
The project's files go to the engine's machine with it, and the playlist's records ride along: the
set list is a **cache manifest** that happens also to be the thing you look at. Any shape that made
that awkward — titles, a database, an order that mattered — would fail at the job it actually does.

Two consequences worth stating:

- A record's stems are the `stems/` folder beside it, so the copy takes that folder too: a record
  copied alone would land with nothing to separate into four platters.
- Copying one set's records never evicts the rest of what the machine holds. Adding a set to a
  machine is additive.

## The refs stay live

A ref names a file of the workspace, and resolves at load, every load. Replace the file — a better
master — and every set naming it plays the new one, with nothing to re-import. This is the same
contract a module ref has, and it is why both are refs rather than copies.

The cost is that a set is not a snapshot: what played last winter is not necessarily what plays
now. The workspace is a repository, so the history holds what was played; a set does not pin it.

## A record the machine has not got is a state, not an error

A ref whose file is not on the machine yet is kept whatever happens. The copy follows what the open
project names, so a set edited a minute ago is briefly ahead of the disk — that is ordinary. It
draws as a record that is **coming**, with its name taken from the path's last segment.

Dropping it from the list would make a set that is missing three records look exactly like a set
that never had them, on the one screen where that difference matters most.

## An entry may carry settings — and that is all it carries

`cues:` is not a table. An entry is **either** a bare ref **or** a ref carrying something:

```k7x
cues([
  "sets/Nirvana/Lithium.mp3",
  ref("sets/Nirvana/Smells Like Teen Spirit.mp3").markers([12.5, null, 48.25]),
])
```

Both forms coexist on purpose. A record with nothing to say fits on one line — so all of them,
until somebody drops a cue — and a list of bare refs reads as it is, with no migration. A table
of refs pointing at almost nothing would make every entry look as though something were missing.

`markers` are the desk's eight cue points, positional, in seconds within the FILE:
[apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/cues.md](../../k7-plugins/docs/turntables/cues.md). The file is written by the desk,
mid-set, and `withMarkers` edits it **in its text** — the scaffold's comments are the only worked
example a reader gets, and a desk that ate them on the first cue dropped would not be forgiven.
