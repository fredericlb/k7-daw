# Server render: `picture render`

`picture render` produces the same file a browser export does, with no browser left open. The
front's file service drives a headless Chromium that runs the browser export's own code.

## Why a browser on the server

The compositor is WebGL2 and the export is WebCodecs plus mediabunny; reimplementing either
natively would be a second renderer that drifts from the first. Running the same `exportScene` in
headless Chromium makes a server render frame for frame what a browser makes. The price is speed:
a server has no GPU, so WebGL runs on SwiftShader (`--use-angle=swiftshader
--enable-unsafe-swiftshader`), on the CPU.

## The pieces

| file | role |
|---|---|
| `apps/k7-daw-wasm/verbs/src/verbs/picture_render.rs` | parses `[webm\|mp4] [wav\|none] [settings]` (settings through `parseVideoSettings`), starts the work under the background tag `picture render` |
| `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/verb_host/picture.rs` | the `picture` member the verb calls on the server |
| `apps/k7-daw-backend/packages/k7-picture/src/lib.rs` | the render: scene, mixdown, Chromium over the DevTools protocol, `find_chromium` (`apps/k7-daw-backend/packages/k7-file-service/src/shares/picture.rs`) |
| `apps/k7-daw-frontend/packages/song-video/src/sceneFrom.ts` | builds the scene, `use: "export"`, with the job's own media URLs |
| `apps/k7-daw-frontend/packages/ui-eyes/src/renderPage.ts` | the page Chromium loads: fetches the job, decodes the mixdown, calls `exportScene`, posts the result |
| `apps/k7-daw-frontend/packages/song-schema/src/renderSettings.ts` | the one parser of `size=`, `quality=`, `from=`, `to=`, shared with `project export video` |

## The job

`renderPicture`:

1. takes the workspace's **active project** and its picture (`loadVideoProject`, read by the `picture render` verb in the browser Worker); no picture lane is an error;
2. finds Chromium: `STUDIO_CHROMIUM` when set (an error if it names nothing), else the first of
   `/usr/bin/chromium-browser`, `/usr/bin/chromium`, and the macOS Chrome and Chromium app paths;
3. bundles `@studio/ui-eyes/renderPage` with esbuild into one IIFE, **once per process**;
4. builds the scene with `sceneFrom(doc, null, "export", p => "media/" + encodeURIComponent(p))`;
5. renders the mixdown WAV (`ws.exportMix("wav")`) unless the sound is `none`;
6. starts a one-off HTTP server on `127.0.0.1`, port 0, every route under a random UUID path:

| route | serves |
|---|---|
| `/<token>/` | the HTML shell |
| `page.js` | the bundled render page |
| `job.json` | scene, format, audio flag, `from`, `frameCount`, `size`, `quality` |
| `mix.wav` | the mixdown |
| `media/<path>` | workspace media through `ws.mediaPath`, files only, with HTTP Range support (mediabunny reads containers in pieces) |
| `POST progress` | `{ frame, total }`, sent every 30th frame |
| `POST result` | the finished file; its content type picks `.mp4` or `.webm` |
| `POST error` | a failure message from the page |

7. launches Chromium through `playwright-core` (headless, the found executable), opens the page,
   awaits the result (a page error fails the job), and writes `exports/<project>.<ext>`;
8. always closes the browser and the server and removes the temporary mixdown.

The token path is the page's only credential: every URL the scene names is under it, so the page
needs no account and no cookie. A stream error mid-file ends that response, never the service.
`STUDIO_RENDER_DEBUG` logs every request the page makes.

## Running it

The command answers immediately ("rendering the picture in the background …") and a second render
while one runs is refused. The result is visible as the new file in `exports/`; progress appears in
the file service's log. Install Chromium (or Chrome) on the file
service's machine or set `STUDIO_CHROMIUM`.

The behaviour is pinned by `apps/k7-daw-wasm/verbs/tests/pictures_media.rs`, which checks the verb's
parsing, background tagging and refusals against a fake host rather than launching a browser.
