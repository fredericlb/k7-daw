# External resources — what the project imported, to hear and place again

`components/ResourcesPanel.tsx`, opened by the toolbar's Resources button or `ui resources`, lists the open
project's imported audio and video and lets them be previewed and dragged onto a lane. The listing
is the file service's `GET /resources` (`apps/k7-daw-backend/docs/file-runtime.md`).

## The unit is the clip, not the file

A row is a `clips/<id>.k7x`, not a media file: a lane places a clip by id (`event add`, `picture
add`), and a clip carries what a file cannot — its window, its warp, a video's extracted sound and
its light copies. Listing files would offer things no verb can place, and the same file twice when
two clips window it. The id is what the row shows, being what every command names.

## Preview plays in the browser, not on the engine

The engine's audition (`POST /preview`) is a one-shot capped at thirty seconds with no stop, and it
plays no picture. A preview here is a native `<audio>` or `<video>` over `mediaUrl`, under its row,
with controls: it seeks, it stops, and it shows a video. One is open at a time, and it exists only
while open — unmounting it is the stop, so closing the panel silences it. A video plays its proxy or
optimized copy when the clip has one, since the master may be a codec the browser cannot decode.

## Dragged onto a lane, it is the PLACING half of a drop

The bytes and the declaration already exist, so a drop runs only what a file drop ends with
(`hooks/useTimelineDrop.ts`'s `onDropResource`): `event add` for audio, and for a video `picture add`
with its measured length, then `picture sound` and `picture prepare`, as a fresh video gets. The
kind travels in the drag's TYPE (`resourceMime`, `fileDrop.ts`), since a `dragover` can read the
types and not the data: a lane of the wrong kind refuses the drop before it is let go, and a video
asks for a picture lane by name.

## And from the lane's menu, for a keyboard and a phone

A drag starts from neither a keyboard nor a phone's web view, so a lane's menu also lists the resources
of its kind, one row each (`placeResource.ts`): "Place <id>" on a track, the same on a picture lane. Picking
one runs `onDropResource` with the caret's timecode, so the two roads end in the same commands. The rows
join the menu when the listing arrives, since the menu never waits on the file service.

## What is not listed

A recorded take lands in `clips/` too and is left out: it was played, not imported, and a project
with a session of takes would bury its imports under them. Stills and captions are picture material
with nothing to hear or watch play. A clip whose file is gone stays listed, marked, since it is
still placed somewhere and the user needs to see it is broken.

## One panel, two segments, filtered rows

References are a segment of this panel (`references.md`), not a second button: both are material
heard beside the project. The rows filter by All, Audio and Video; there is no "records" filter
because a clip does not say how it was made, and the id is all the listing knows.
