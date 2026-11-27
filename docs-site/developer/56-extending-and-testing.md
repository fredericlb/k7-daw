# Extending the picture, and testing it

## Recipes

### A new field on a shot

1. Add it to `PlacementSchema` in `apps/k7-daw-frontend/packages/song-schema/src/yaml/placementSchema.ts`, the one event
   schema sound and shots share. The schema is `.strict()`, so an unknown key is refused on its
   line; a shot-only field goes in `SHOT_KEYS` (refused on sound), and a field that needs a source
   in the list the `superRefine` refuses on an activation.
2. Carry it in the document's `Clip` (`apps/k7-daw-wasm/verbs/src/picture/doc.rs`) and read it in
   `clip_of` (`apps/k7-daw-wasm/verbs/src/picture/lanes.rs`). Keep "absent stays absent".
3. Mirror it in `WebVideoProject` (`apps/k7-daw-frontend/packages/web-protocol/timelineTypes.ts`), in `SceneClip`
   (`apps/k7-daw-frontend/packages/ui-eyes/src/scene.ts`) and in the scene builder (`apps/k7-daw-wasm/verbs/src/picture/scene.rs`).
4. Use it in `apps/k7-daw-frontend/packages/ui-eyes/src/timeline.ts` when it changes *which* frame shows, or in
   `slots.ts` / `motion.ts` when it changes *how* a slot is drawn — every driver goes through those.
5. Decide what the writers must do with it: `splitVideoPlacementAt` (which half keeps it),
   `apps/k7-daw-wasm/song/src/pictures/sound.rs` (does the sound follow it), and
   `apps/k7-daw-wasm/verbs/src/verbs/picture_interchange.rs` (carried, or listed as left out).
6. If it is editable by command, add a verb (below) and a writer in
   `apps/k7-daw-frontend/packages/song-schema/src/yaml/videoTrackFile.ts` that edits by index and rounds as the others do.

### A new `picture …` verb

Declare it in `apps/k7-daw-wasm/verbs/commands.yaml` in the picture section: `command`, `sid`,
`readable` in `en` and `fr` (60 characters at most each; the spec test enforces it), `help`,
`surfaces: [cli, timeline]`, `args` with `complete: pictureLanes` for a lane, and `menu: [shot]`
if a shot's right-click should offer it. Implement the handler beside the others in `apps/k7-daw-wasm/verbs/src/verbs/`
(`picture_shared.rs` gives the addressing and the refusals), list it in `apps/k7-daw-wasm/verbs/src/verbs/mod.rs`, and
test it on its effects (`docs/verbs-port.md`). Work that takes long is a route the verb calls (`h.call`).

### A new uniform for shaders

Add it to the shader list in `apps/k7-daw-frontend/packages/ui-eyes/src/passes.ts` and set it in the pass-drawing
function there; extend `ScenePass` if it comes from the document. The list is the contract every
shader author reads, so keep its comment the one-line definition of the value.

### A new shape kind

Add the kind to `SHAPE_KINDS` in `apps/k7-daw-frontend/packages/song-schema/src/moduleTypes.ts`, a branch in `shape_of`
(`apps/k7-daw-wasm/verbs/src/picture/chains.rs`), the matching `SceneShape`, and its drawing in
`apps/k7-daw-frontend/packages/ui-eyes/src/rasterize.ts`. Geometry belongs in `params`, as for the existing kinds.

### A shipped shader or transition

A shader is a module folder in a library: `module.k7x` (`type("shader")`, `shader("<name>")`,
`params` P1..P4), `shaders/<name>/vertex.glsl` and `fragment.glsl`, and optionally a `ui.k7x`
panel binding knobs to `P1`…`P4`. `k7-core/modules/shaders/grade/` and
`k7-core/modules/shaders/look/` are the references. A transition is a `video_transitions/<name>`
module with its two stages directly in the module folder, reading `uFrom`, `uTo`, `uProgress`.

### A new project archetype field

The `video` and `live-video` file maps are `videoProjectFiles` in `apps/k7-daw-wasm/song-edit/src/scaffold.rs`
and `liveVideoProjectFiles` in `apps/k7-daw-wasm/song-edit/src/scaffold_live.rs`; both splice the base
archetype's graph rather than restating it, and `add_video_run` (`apps/k7-daw-wasm/song-edit/src/video_outputs.rs`)
adds the `RENDER → VIDEO_OUT` run.

## Tests

The TypeScript packages here use vitest (`npm test` in the package, `vitest run`), the compile and the scene `cargo test` in the
verbs crate. From the root, `npm test` runs every workspace. Nothing here needs a GPU, a camera or ffmpeg: the GL-free halves
are split out precisely so they can be tested.

| area | tests |
|---|---|
| schemas and writers | `apps/k7-daw-frontend/packages/song-schema/src/yaml/videoTrackFile.test.ts`, `videoTrackYaml.test.ts`, `videoKeys.test.ts`, `videoSpeed.test.ts`, `apps/k7-daw-frontend/packages/song-schema/src/aspect.test.ts` |
| the compile and the scene | `apps/k7-daw-wasm/verbs/tests/picture.rs`, over `e2e/fixtures/video-lane.k7` and the crate's `tests/fixtures/picture-rig.k7` |
| interchange | `apps/k7-daw-wasm/verbs/tests/pictures_media.rs` (writers, readers, round trips) |
| song side | `apps/k7-daw-wasm/song/tests/all/pictures_sound.rs`, `pictures_doc.rs` |
| renderer | `apps/k7-daw-frontend/packages/ui-eyes/src/timeline.test.ts`, `compositor.test.ts`, `passes.test.ts`, `motion.test.ts`, `text.test.ts`, `cube.test.ts`, `camera.test.ts`, `exportSupport.test.ts` |
| plugin | `viewportRect.test.ts`, `shaderAudio.test.ts` in `apps/k7-daw-frontend/packages/k7-plugins/src/video/` |
| verbs | `apps/k7-daw-wasm/verbs/tests/pictures.rs`, `pictureSpeed.test.ts`, `pictureSound.test.ts`, `pictureMedia.test.ts`, `pictureInterchange.test.ts`, `pictureImport.test.ts`, `pictureRender.test.ts`, `projectAspect.test.ts` |
| media preparation | `apps/k7-daw-wasm/verbs/src/verbs/picture_media.rs`, `transcode.test.ts` (the ffmpeg arguments, not a run) |
| frontend | `videoDrag.test.ts`, `videoSpans.test.ts`, `videoTake.test.ts`, `timelineDrop.test.ts` |

Two tests pin rules that live in two places: `apps/k7-daw-frontend/src/videoSpans.test.ts` holds the
graph's "lane is on screen" spans against the compositor's `activeAt`, and
`apps/k7-daw-wasm/verbs/tests/picture.rs` holds the mute/solo/lens fold the scene draws. When you change a rule in `ui-eyes/src/timeline.ts`, run both.

The demo project `demos/video-demo.k7` exercises lanes, a picture-in-picture, shapes, a title, a
stretch fade (`iris`) and an audio-reactive shader (`glow`); its `counter.mp4` burns the frame
number into the picture, so sync and `from` can be checked by eye.

Every edited `.ts` file must pass biome's lint (`npm run lint` at the root).
