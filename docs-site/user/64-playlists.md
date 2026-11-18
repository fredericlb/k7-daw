# Playlists

A **playlist** (a set list) is an ordered list of records for a DJ desk. A record is an audio file of your workspace, named by its path from the workspace's root.

Playlists live in `_playlists/` at the root of a workspace, as `.k7x` files like every workspace file. They are kept with your projects: the same whichever project of the workspace is open. A desk (`surfaces/desk.k7x`) names one, and the machine running your engine receives copies of its records ahead of time.

## The file format

A playlist is a `.k7x` file under `_playlists/`. Its name is its path without the extension: `_playlists/ete-2026.k7x` is the playlist `ete-2026`, `_playlists/live/repet.k7x` is `live/repet`.

```k7x
// ete-2026
title("Été 2026")
items([
  "records/AC-DC/Hells Bells.flac",
  "records/Daft Punk/One More Time.wav",
])
```

| Field | Meaning |
|---|---|
| `title` | Optional. What lists show; without it, the file name. |
| `items` | The records, in order. |
| `items[]` as a string | A record: the path of an audio file in the workspace, from its root. |

- The same record may appear twice (an encore, a segue played again). The desk's browser lists it once per line.
- A playlist that does not parse is not treated as empty: it is reported as broken.

## The playlists panel

Open it with the list icon in the footer (shown in projects that have a desk) or with `ui playlists` from the command bar, in any project. It docks on the right.

| Part | What it does |
|---|---|
| Dropdown at the top | Picks which playlist the panel shows. Click it to search the workspace's playlists by name or path. |
| `+` | New playlist: type a name (the extension is added) and press Enter or **Create**. It runs `playlist new <name>`. |
| Expand icon | **Edit the file (text + records)**: opens the full editor, the text on the left and the records on the right. |
| `×` | Closes the panel. |
| The list | The records, each shown by its file name and its folder. |

A warning "this file does not parse — open the text to repair it" means the file needs fixing in the full editor.

### The record list

Each row is one line of the playlist, numbered from 1.

| Action | How |
|---|---|
| Listen to a row | Click its play button. Playback is in the **browser**, not on the engine; the rows play one after another. Click again to pause. |
| Add records | The search icon opens a search over the audio files of the workspace. Click a file to append it. Enter adds the first hit. Escape closes. At most 40 hits are drawn; narrow the search for more. |
| Reorder | The up/down arrows on a row. |
| Remove | The cross on a row. |
| Put on a deck | Drag the row onto a deck of the desk. |
| Separate the stems | The **Separate the stems** button opens a bar that prepares every record; see [Preparing a gig](./65-gig-prep.md#separating-stems). |

A row that cannot play says why: the file is not in the workspace, or it is not an audio file. A row already separated says so too.

### The full editor

The expand button opens the playlist as text on the left and the resolved list on the right. There is no Save button: the text is written as soon as it reads as a playlist. Completion offers `title`, `items` and the audio files of the workspace. If the text stops parsing, the list keeps showing the last readable version.

When `items` is written in a shape the panel cannot extend, adding, removing and reordering are refused with a message asking you to change the line by hand.

## Commands

These work from the command bar with no project open, except `playlist stems`.

| Command | Effect |
|---|---|
| `playlist new <name>` | Creates `_playlists/<name>.k7x` with an empty list. Sub-folders are allowed (`live/repet`). Refused if it exists. |
| `playlist song add <playlist> <file>` | Appends a line. The record is the rest of the line: the path of an audio file of the workspace. |
| `playlist song remove <playlist> <n>` | Removes line `n`, counting from 1. |
| `playlist song move <playlist> <from> <to>` | Moves a line; both positions count from 1. |
| `playlist stems <playlist> [chain]` | Separates every record of the playlist into stems. See [Preparing a gig](./65-gig-prep.md#separating-stems). |

Examples:

```
playlist new ete-2026
playlist song add ete-2026 records/AC-DC/Hells Bells.flac
playlist song move ete-2026 5 1
playlist song remove ete-2026 3
```

Positions are used rather than file names because a playlist may name the same record twice.

## Naming a playlist on a desk

In `surfaces/desk.k7x`:

```k7x
type("turntables")
layout("split")
playlist("ete-2026")
```

The desk's record browser then lists exactly that playlist's records, the playlists panel's rows can be dropped on its decks, and the machine running your engine receives copies of those records ahead of time (see [Preparing a gig](./65-gig-prep.md)). Only a `turntables` surface accepts `playlist`.
