# The reference track, and why the browser plays it

A **reference track** is a finished recording you listen against while you work. It is a file in
`_references/` at the root of the workspace and nothing else — what one is and where it lives is
`@studio/song-schema`'s `docs/references.md`. This page is the editor's half: who plays it, and
where it is drawn.

## It is NOT played by the engine, and that is the whole point

The requirement is that a reference sounds **while the rig or the arrangement is playing** — you
put your mix and somebody else's next to each other, switching between them, and the transport
never stops. The engine cannot serve that:

- its transport belongs to the PROJECT. Playing a reference through it would mean starting and
  stopping the thing you are trying to hear;
- a reference is not in the project and must never get there. Anything the engine plays it plays
  from a `ProjectDoc`, so "just add a clip" is a track in somebody's song, on their master, in
  their export;
- a live archetype has no arrangement at all — nothing to place a clip ON.

So the `<audio>` element in `ReferenceBar.tsx` plays it, straight out of the file service's
`/media` route, and it owes the engine nothing: no session, no document, no transport. Press play
during a take and the take is untouched.

**What that costs, and the bar says it out loud:** the sound comes out of the *browser's* output
device, not the audio interface the engine holds. On a rig where the interface is not the system
default they land in different places, which is why the row reads "this browser's output" rather
than leaving you to work it out from what you hear. Nor is it synchronised with the playhead:
you are comparing tone, not timing.

## Two views, and the bar is the one that outlives everything

`ReferencesPanel` is a listing — it plays nothing. `ReferenceBar` holds the element. That split
is the mini player's, for its reason ([apps/k7-daw-frontend/packages/ui-mini-player/docs/miniPlayer.md](../packages/ui-mini-player/docs/miniPlayer.md)): a
transport owned by a panel dies when the panel closes, and closing the list of references is
exactly what you do before listening.

The bar is mounted above the status footer, **outside every tab and every pane**, so switching
from the timeline to the chains graph, opening a surface or resizing the bottom panel does not
interrupt it. `App` holds the one piece of state that matters — which track is loaded — and
`null` is what removes the bar. A track that ends leaves it up, reading "ended".

One track at a time: choosing another replaces what is loaded. Two references at once is not
listening, it is a mix. The panel CHOOSES and the bar PLAYS, which is the whole of the split — a
row loads its track and starts it, and everything after that (pausing, scrubbing, starting it
again) is the bar, sitting where you can reach it with the list closed.

## The row is `@studio/ui-mini-player`

See `apps/k7-daw-frontend/packages/ui-mini-player/docs/miniPlayer.md`.
It owns no transport, which is what lets one component serve an app playing over several
destinations and an app playing one file in a tab.

**Its wave keeps its slowest crawl here**, like the project bar's while the engine is stopped: a
reference plays in this browser, and the engine's meter says nothing about it. Measuring the
element would mean routing it through Web Audio, and a background is not worth a sound that an
autoplay policy can take away.

## The listing is the disk, re-read

Upload, delete, and the panel asks `/references` again rather than patching the array it holds.
The folder is an ordinary folder: a git pull writes into it from another device,
somebody drops a file in from the OS, a replica joining brings twelve. A local list that "knows better" would be wrong within the day, and there is
nothing here worth optimising — the route stats a folder.

Uploads go through the same `/upload` every other file uses, into `_references`. There is no
route of its own, no import step and no sidecar to write.

## They travel with the workspace

`_references/` is inside the workspace, so it travels to every device like the
song's own files. That is the point of the folder being where it is:
open the workspace on the machine at the venue and what you were mixing against is there. It is also worth knowing before you drop somebody's whole
discography in.

## A segment of Resources

The list of references is the Resources panel's second segment; its `+` sits in a toolbar row of
its own, since the panel frame owns the title and the Close button.
