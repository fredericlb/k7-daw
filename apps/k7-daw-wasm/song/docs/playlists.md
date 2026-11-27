# A playlist is a file — in the repository

A playlist is an ordered list of **records**: audio files of the workspace, each named by its
workspace-relative path. It lives under `_playlists/`, at the root of the workspace, at whatever
depth you like:

```
<workspace>/_playlists/ete-2026.k7x
<workspace>/_playlists/live/thursday-rehearsal.k7x
```

```k7x
// _playlists/live/thursday-rehearsal.k7x
title("Thursday rehearsal")
items([
  "sets/AC-DC/Hells Bells.mp3",
  "sets/Nirvana/Lithium.mp3",
])
```

**A playlist is the account's document, written for one specific piece of work.** A gig's setlist
belongs to the repository holding the records played at it — under `_playlists/` it commits,
branches and clones with them. That is also what lets a `turntables` surface NAME one: the desk
resolves `playlist:` in the workspace, and the engine's machine receives the records it finds
there with the project.

**The records themselves stay put.** The file that names is ours, the bytes named are the
workspace's, and a record's stems are the `stems/` folder beside it
(`apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/stems.md`). A playlist is prepared once for the night with
`playlist stems <playlist>`, which cuts record by record.

## `_playlists`, beside `_modules`

Same **kind** of folder, so same place and same rule: found by where it is, never by a line
declaring it (`docs/sharedFolders.md`). A playlist is not a project and has no business inside one
— you build a setlist once and open it from any project in the workspace.

Hence `Workspace.playlists()` working at the workspace root and not beside the active project. A four-song workspace would otherwise have four separate playlist folders,
which is exactly what a playlist is not.

And hence its being a **separate route** (`GET /playlists`) rather than a corner of `/tree`: that
tree is rooted on the active project, deliberately
(`apps/k7-daw-backend/docs/file-projects.md`), so no walk of the open song can reach this
folder. Same reason as `/module-libraries`.

## The file IS the entry

There is no folder around a playlist, and no `playlist.k7x` marker inside one: **the extension is
the rule** (`PLAYLIST_EXT`), where a song, a chain and a module are folders because each carries
what its file names — a master, its nodes, its assets. A playlist carries only pointers; a folder
around it would be an empty box.

## Two spellings for one line

A line is either a string or a mapping:

| what you write | what it means |
|---|---|
| `- sets/Nirvana/Lithium.mp3` | that record |
| `- {song: sets/Nirvana/Lithium.mp3}` | the same, the long form |

The key stays `song`: the file format is older than the word "record", and renaming it would
break every playlist written. The mapping may carry a `variant` key, which the parser keeps and
the desk and the pane **ignore** — a record is a file, with nothing to choose among. A line
naming **no** record is thrown away rather than salvaged: `- [x]` is a badly written line, and
manufacturing `"[object Object]"` as a reference would put a row on screen pointing at nothing you
can go and look at.

The reference is the record's path **in the workspace**, slashes trimmed, never leaving it
(`recordPathOf`): a line holding a `..` segment, a backslash or a leading `@` names no record.

## References are not checked here

`parsePlaylist` keeps `song` **verbatim**. Resolving a reference means holding the workspace's file
list, and this file opens nothing — not even `node:fs`: that is what lets the file service and the
browser go by exactly the same rules.

So it is the job of whoever holds both lists — k7's playlist pane
(`apps/k7-daw-frontend/docs/playlists.md`) and the desk (`apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/setList.md`),
each with the playlist on one side and the files on the other. They share the resolution itself
rather than each having a copy: `resolveRecords` in `@studio/k7-wasm/records`, since two
copies would diverge at the first case learned on one side only.

## What the listing reads anyway

Two things, neither load-bearing: `title:` for what the list displays, and the item count. With no
title, the filename serves.

A file that **does not parse** is listed all the same, with `ok: false`. It is on disk: a listing
that hid it would leave you unable to click the very thing that needs fixing. `parsePlaylist`
returns `null` in that case, not an empty playlist — the two are different facts and a surface
says them differently: a file that does not parse is to be repaired, an empty `items:` is to be
filled.

## Writing a line without going through the serialiser

`appendPlaylistItem` adds a line **to the document**, by key path. Same discipline as `yaml/edit.ts`
takes with a project's files, for the same reason: these files carry comments and an order somebody
chose, and round-tripping through an object would throw both away silently.

It returns `false` — rather than an approximation — when `items` is not a list. The surface then
disables its button and says why; writing a second `items` further down would produce a file whose
two halves contradict each other.

## Creating, and adding a line

Two **commands**, `playlist new <name>` and `playlist song add <playlist> <record>`
(the verbs crate (`apps/k7-daw-wasm/verbs`)) — the only ones in the catalogue acting on the repository (`repoDir`) rather
than on a project, which is exactly what a playlist is. They work with no project open.

`playlist new` writes the **template**, not an empty file: the screen draws its right-hand pane
from the text, so an empty file would open on a parse failure for a playlist nobody has yet had
the chance to get wrong. It adds the extension if missing — the extension is what makes the file —
and refuses a name already taken rather than opening it: a "create" that quietly handed back
someone else's setlist is how you edit the wrong one.

`playlist song add` goes through `appendPlaylistItem`, so through the document: the comments and
the order written are still there afterwards. A shape the append cannot extend is refused by name,
never worked around by writing a second `items` further down.

The name goes through `playlistPathOf`, which refuses a `..` segment — and the verb additionally
checks that the RESOLVED path stays under `_playlists/`, because that function splits on `/`:
`..\..\x` is one segment to it and three to `join` on Windows. It is the sentence `safePath` says
on the file-service side, said here for a path coming from a command line.
