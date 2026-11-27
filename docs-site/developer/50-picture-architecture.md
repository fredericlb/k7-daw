# The picture pipeline: architecture

A song's picture — video lanes, shots, shaders, titles, subtitles, cameras — is compiled, drawn
and rendered by a pipeline that runs **beside** the audio one and never inside it. This page is
the map; the following pages go package by package.

| page | covers |
|---|---|
| this page | the shape of the pipeline, the decisions behind it, where everything lives |
| [the picture compile](./51-song-video.md) | the picture document, the lane and chain compile, shaders, captions, the scene |
| [ui-eyes](./52-ui-eyes.md) | the WebGL compositor, realtime playback, frame-exact export |
| [Viewport plugin and frontend](./53-viewport-and-frontend.md) | `k7-plugins/src/video`, the timeline's picture lanes, drops, camera recording |
| [Picture commands and media preparation](./54-picture-commands.md) | the `picture …` verbs, the file writers, `video-prep`, a shot's sound |
| [Server render](./55-server-render.md) | `picture render`: headless Chromium driven by the file service |
| [Extending and testing](./56-extending-and-testing.md) | adding a field, a uniform, a shape, a transition; the tests |

## Data flow

```text
project folder (project, picture lanes in nodes/ or tracks/, clips/, graph or chains/, modules)
   │  compile()                          apps/k7-daw-wasm/verbs/src/picture/compile.rs
   ▼
Doc               (seconds, GLSL inlined, warnings; paths workspace-relative on POST /picture)
   │                                     apps/k7-daw-frontend/packages/web-protocol/timelineTypes.ts (WebVideoProject)
   ▼
   │  scene_of()                         apps/k7-daw-wasm/verbs/src/picture/scene.rs (POST /picture/scene, and picture render)
   ▼
VideoScene        (plain structural type, media URLs, mute/solo/lens already folded)
   │
   ├─ realtime.ts   → the viewport, against the engine's playhead
   └─ export.ts     → a webm/mp4 file, frame by frame (browser, or headless Chromium on the server)
```

The engine is never handed any of it. The C++ engine plays the song's sound, including a shot's
sound through an ordinary audio lane (see [Picture commands](./54-picture-commands.md)), and reports
its playhead and meter levels; the browser does everything picture.

## Decisions and what forced them

**A second document, not a section of `ProjectDoc`.** The picture is a second compile of the same
folder (the Worker's `videoDoc` beside `/project-doc`). A song has a tempo; its picture has a viewport,
and the engine must never receive the latter's fields. It also lets a song's sound compile without
loading any of `song-video`, which is why that package was split out of `@studio/song`.

**Seconds everywhere.** Picture is cut against the wall clock. Every time in a lane file and in
`VideoProjectDoc` is a decimal second; the timeline converts with the song's single tempo
(`beats per second`), which is exact because there is one tempo. Changing the tempo slides the bar
grid under a shot rather than moving it.

**The renderer knows nothing of the brain.** `@studio/ui-eyes` imports none of the Node packages:
it takes a `VideoScene` and a clock. The adapter from the document to the scene lives in
`k7-plugins/src/video`, and its browser-free half (`song-video/src/sceneFrom.ts`) is shared with the server render,
so a browser export and a server render build the very same scene.

**One renderer, three drivers.** Realtime playback, browser export and server render all run the
same `Compositor` and the same timeline rule (`ui-eyes/src/timeline.ts`). WebGL's `texImage2D`
takes an `HTMLVideoElement` and a `VideoFrame` alike, so the compositor never learns which driver
feeds it.

**GLSL travels inline.** The compile reads every shader, transition and stretch program off disk
and carries the source in the document: the browser cannot open a module folder.

**Cameras are the browser's.** A lens is opened with `getUserMedia`, recorded with `MediaRecorder`,
and its arm state is per browser. The engine never hears about a camera.

**Files are edited surgically.** Lane files carry comments; every writer in
`apps/k7-daw-wasm/song-edit/src/picture_file.rs` changes values in place rather than
re-serialising the file, and rounds times to the millisecond so a drag and a verb leave the same
bytes.

## Where the code lives

| path | role |
|---|---|
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/entitySchemas.ts` | `VideoClipSchema`, `CaptionClipSchema` |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/placementSchema.ts` | `PlacementSchema` (one event, a sound event or a shot), `FadeSchema`, `SHOT_KEYS` |
| `apps/k7-daw-wasm/verbs/src/picture/lanes.rs` | which lanes are picture lanes (`@meta/picture-track`), and each one's file |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/projectSchema.ts` | `viewport` (draw order is `trackOrder`'s picture rows, top in front), the `video` and `live-video` archetypes |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/moduleSchema.ts` | the picture module fields: `shader`, `shape`, `font`, `align`, `background`, `shadow`, `camera`, `frame`, `lut`, `video_source` |
| `apps/k7-daw-frontend/packages/song-schema/src/moduleTypes.ts` | `RENDER`, `VIDEO_OUT`, `CAM`, the `shaders` / `shapes` / `video_transitions` / `video_effects` categories, `SHADER_PARAMS`, `DISSOLVE`, `OVER` |
| `apps/k7-daw-frontend/packages/song-schema/src/keyframes.ts` | the `animate` schema and its evaluation |
| `apps/k7-daw-wasm/song-edit/src/track_file.rs` | surgical writers for lane files |
| `apps/k7-daw-frontend/packages/song-schema/src/aspect.ts` | the `16:9` / `9:16` shapes |
| `apps/k7-daw-wasm/verbs/src/picture/` | the picture compile and its scene |
| `apps/k7-daw-wasm/song-edit/src/video_outputs.rs` | writing the `RENDER → VIDEO_OUT` run into a project that lacks it |
| `apps/k7-daw-wasm/song/src/pictures/sound.rs` | a picture lane's sound clips derived from its shots — its own `#audio` half, or a chain project's `video_source:` lane |
| `apps/k7-daw-wasm/verbs/src/picture/mod.rs` | `has_picture` |
| `apps/k7-daw-wasm/song-edit/src/scaffold.rs`, `apps/k7-daw-wasm/song-edit/src/scaffold_live.rs` | the `video` and `live-video` archetypes' files |
| `apps/k7-daw-frontend/packages/ui-eyes/` | the WebGL compositor, realtime and export |
| `apps/k7-daw-frontend/packages/video-prep/` | ffprobe / ffmpeg: optimized copies and proxies |
| `apps/k7-daw-frontend/packages/k7-plugins/src/video/` | the viewport surface, the scene adapter, picture mute/solo/lens rules, the camera relay |
| `apps/k7-daw-wasm/verbs/src/verbs/picture*.rs` | the `picture …` verbs, OTIO/EDL among them |
| `apps/k7-daw-backend/packages/k7-picture/src/lib.rs` | the server render |
| `apps/k7-daw-frontend/src/` | the timeline's picture lanes, drops, camera recording, `project export video` |

