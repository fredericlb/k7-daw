# The picture compile

The verbs crate's `picture/` module (`apps/k7-daw-wasm/verbs/src/picture/`) turns a project folder into the picture
document the page reads (web-protocol's `WebVideoProject`) and into the scene `@studio/ui-eyes` draws. The backend
answers both: `POST /picture` and `POST /picture/scene` (`apps/k7-daw-backend/docs/file-changes.md`); `picture render`
builds the same scene for a film.

| file | role |
|---|---|
| `compile.rs` | `compile`: the viewport, the lanes in draw order, the markers, the timelines' own picture lanes, the cameras that feed no lane |
| `lanes.rs` | one shot as a clip: its media found and confined, its fades and keys normalised, a subtitle clip's cues read |
| `chains.rs` | the graph's picture runs: each lane's passes, the frame's, the lenses, the shapes, the stretches |
| `glsl.rs` | a shader's or a transition's two stages, read off its module folder or an ancestor's |
| `stretch.rs` | each fade's `chain:` and each shot's `effect:` held to the runs landing on its lane |
| `captions.rs` | SRT and WebVTT read into cues |
| `scene.rs` | the scene: lanes folded to shown or hidden, the enabled passes, each file made a URL |
| `doc.rs` | the document's types, as the page reads them |

## The document

| field | content |
|---|---|
| `viewport`, `fps` | from `project.viewport: [w, h, fps?]`; fps defaults to 30 and is used by export only |
| `videoTracks` | the picture lanes in draw order (the picture rows of `trackOrder`, bottom first), then the lane-less camera layers on top |
| `markers` | project markers read as wall-clock points, sorted |
| `framePlugins` | the `RENDER → VIDEO_OUT` run |
| `transitions` | every transition a fade names, by name, GLSL inline; the built-in dissolve is absent |
| `stretches` | every run a fade's `chain` or a shot's `effect` names, by head node |
| `timelines` | each timeline's own picture lanes, framed like the song's lane of their name |

The answer carries `warnings` beside the document. The split between **refusal** and **warning** is deliberate: a
refusal is a picture that cannot be built (no `viewport`; a lane's last open-ended shot fading out), a warning is a
picture that builds and then draws less than asked (a ref with no clip, a shape wired to no lane, shots on a lane of
shapes, an unknown stretch, an unreadable subtitle block). A blank lane must be able to say why.

A lane draws one of three things, never a mix: its `clips`, its `shapes`, or a `camera`. A shot with no `ref` is an
**activation**, a span saying a lane of shapes is on. A number as a fade is `{ over, with: "dissolve" }`, a map with a
`chain` and no `with` blends `over`, a zero-length fade is no fade.

## The runs

The graph is read **best-effort**: an unreadable graph yields no runs, so the lanes still draw. A camera's chain
landing on picture lanes gives each its lens (the first wins); landing on none, the camera is a layer of its own. A
shape is drawn by every lane its chain lands on. A stretch (fed by nothing, landing on a lane) is keyed by its head
node; a chain from `RENDER` reaching `VIDEO_OUT` is the frame's run; a chain from a picture lane is that lane's. A
bypassed step is kept with `enabled: false` in the document, and dropped from the scene.

A shader's stages are under its module's `shaders/<name>/`, a transition's loose in its `video_transitions/<name>/`
folder; both walk the `extends` ancestry nearest first, and a name outside `[A-Za-z0-9_-]` is refused before any path
is built.

## The scene

`scene.rs` folds mute, solo and a lens's monitor into each layer's `hidden`; a preview reads a shot's proxy, then its
optimized copy, then its source, an export never the proxy. The preview's files are the page's `/media` URLs, a film's
are paths the picture route makes `media/` URLs of.

## Tests

`apps/k7-daw-wasm/verbs/tests/picture.rs` reads `e2e/fixtures/video-lane.k7` and the crate's own
`tests/fixtures/picture-rig.k7` (runs, a lens, shapes, captions, a transition, a timeline's pictures) and asserts on the
document and the scene; the interchange verbs are `tests/pictures_media.rs`.
