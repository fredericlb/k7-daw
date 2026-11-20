# The file service in the front

An account's routes are answered in the front, by native pieces and nothing else (`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/`;
the pieces are in `file-native.md`). No JavaScript runtime exists for an account: a route no piece
answers is refused with a sentence (`declines.md`), and nothing falls back to anything.

## The account

| choice | why |
|---|---|
| an account is made on its first request (`services.rs`) and holds its song sessions and environment (`accounts.rs`), its log, its code root, the runtime state (the Logs ring, the background registry, the machine's asks), the members, the native capture, the admin stamp, the media and the engine link, in one `Held` | one owner per account for what the routes share; no thread and no runtime per account, so an idle account costs memory only |
| an account lives until the front stops | an idle one let go would make the next open pay a cold start |
| the account's environment is the one its service would be started with (`Instances::environment`), with a secret of its own that the relay, the libraries' calls and the shares' internal routes take as a running service's (`hub::State::account_of_secret`) | the relay knows an account by its secret |
| the song sessions' heap budget is an embedded host's, 192 MB, a quarter of it for the sessions; no environment variable sets it | a phone and the desktop app share memory with the person's other programs |
| a route the table does not name is answered thus: the owner (`owner.json`), the workspace header, then Express's 404 page (`Cannot <METHOD> <path>`, `routes/last/express.rs`); `GET /health` is its ungated `{ ok: true }` | every route a client sends is in the table |
| a request naming an engine binds the ACCOUNT (`k7_hub::binding::Bindings`), the engine link reading the binding from there | the binding is made in the request's path |
| `findEngineBinary` and the translator's one process are answered by the front (`machine.rs`) | an import asks both |
| the runtime report line is `[files] native runtime: background=<n> logs=<n>`, judged by `scripts/agent/bedCounters.mjs` with a floor `logs>=1` | the proof that the native runtime state is the one in use is a counter that moved |

## The live service

| choice | why |
|---|---|
| the capture and the ledger are native (`native/capture*.rs`); the front gives them recursive watchers (`notify`) | the capture is the one writer of a workspace's log and history |
| an account whose env names `K7_NO_FS_WATCH` gets a watch that never fires, and no `notify` watcher is made; only the suites that compare answers and disks set it | a recursive watcher on macOS blocks until fseventsd starts its stream, and a Mac busy with builds keeps it for minutes |
| `Watch::silence(folder)` makes the process's watcher skip every root under a folder | a test reaches `Revisions::global` from the front's own routes, so the account's switch never arrives; a folder is the narrowest scope, and a silenced root's token is walked at every ask |
| an upload runs inside the capture window and a client that hangs up closes it (`window.rs`) | the bytes and the rename are one batch of the capture |
| the engine `Bridge` (`native/engine/host.rs`) is the link's `Tells` only: files takes, says lines | the engine link's logic is native; the bridge only carries what it says |
| the account's binding is kept under its key by the hub, whatever serves the account | an account made again comes back to the same URL; the engine's copy of the account stays where it was |
| a socket dialled with headers (the relay's) has its frames held while a verb writes; the engine's and the jobs' never are | a verb runs for minutes, and a take, a state frame or a render's reply must not wait for it |
| an undo's missing audio blob is fetched with the bound machine's `send-up` job (`native/journal_undo.rs`) | the blob lives on the machine that recorded it |
| TLS for outbound calls is `k7-front/src/trust.rs`; multipart bodies (`form.rs`) and the media's direct half live in the front | one trust store for every outbound call of the front |

## The media

The media the file service reaches is `src/media/`: the waveform module runs in Wasmtime (`k7-mediatek`), and programs are started by the front.

| choice | why |
|---|---|
| the queues are bounded: two analyses, two transcodes and 64 waiting, a phone's decodes one at a time | the bounds are node's; the front's threads are tokio's blocking pool |
| a decode is stopped between blocks when its signal aborts; a transcode past its deadline fails with a deadline error | there is no worker to terminate; a block is a few milliseconds |
| an analysis decodes through the engine's `--decode` when an engine is named, else the module, then ffmpeg for what it cannot open (`k7-mediatek/src/decode.rs`) | the engine's mp3 is longer by its encoder delay (`apps/k7-daw-frontend/packages/audio-analysis/docs/analysis.md`) |
| the front spawns ffmpeg, ffprobe, `whisper` and `whisper-cli` only, as the account's `PATH` finds them (a path is taken only when it is that very file; the engine also from the checkout's `apps/k7-daw-engine/k7-runtime/build`), or what `FFMPEG_BIN`/`FFPROBE_BIN` name, with the account's environment and nothing else, the front's own cleared | any other program is refused as a spawn error |
| an envelope missing its sidecar is decoded through the front's gate at the default threshold, another threshold ungated | the front's sweeps and this route then share one decode per file |
| a route's `sendFile` answers the file's path; the front streams it with Range and runs the route's callback once the body ended or the client hung up (`service/sent.rs`) | minutes of audio never sit in memory; the temp folder is removed after the bytes went |
| a release's mixdown and stems and a share's file are streamed from disk as one multipart body with its length said (`k7-front/src/form.rs`) | the bytes never enter memory |
| a request that sends a file or a form may take 30 minutes to answer, any other 60 s; a body that sends nothing for 300 s fails | an upload's answer comes after its bytes |
| a model's download past 4 MB is written by the front into the job's `.part` file, telling the length of each block | 166 MB never sits in memory; a model's address redirects to its CDN |
| the true peak a cold pass measures is the engine's `--decode` read natively, a running maximum over every channel | the same maximum over the same decode as node's |

### Picture render

| choice | why |
|---|---|
| `picture render` makes the scene, the settings and the mixdown natively, then hands chromium the job (`apps/k7-daw-backend/packages/k7-picture`) | decision 10: the image's chromium driven over the DevTools protocol by `chromiumoxide`, Playwright being node's |
| the browser is `STUDIO_CHROMIUM`, else the newest Chromium Playwright installed (every platform's `ms-playwright` folder, `PLAYWRIGHT_BROWSERS_PATH` first), else the system's | a dev machine and the Mac's beds have Playwright's browser and no system one; the front looks in every platform's folder |
| WebGL draws on SwiftShader where the browser ships it (`libvk_swiftshader.so`, `vk_swiftshader.dll` beside it: Playwright's), else, on Linux, on Mesa's lavapipe through Vulkan (`--use-angle=vulkan --ignore-gpu-blocklist`); the image installs `mesa-vulkan-swrast` and `vulkan-loader` (`apps/k7-daw-backend/packages/k7-picture`) | Alpine's chromium ships no SwiftShader, so `--use-angle=swiftshader` gave the render page no WebGL2 and every render failed; lavapipe is a CPU device, which chromium's GPU blocklist refuses unless told |
| the page fetches from a one-off loopback server under a random path, which serves the page, `job.json`, the mixdown and only the media the scene names, each resolved by the workspace before | the page needs no account; nothing else of the machine is reachable through it |
| the render page is built beforehand by `k7-picture/page.mjs`: by the image's build into `K7_RENDER_PAGE`, by hand in a checkout (`apps/k7-daw-backend/packages/k7-picture/page/`, ignored); without it the verb says how to build it | the plan's "no esbuild at run time"; a committed copy would drift with every change to the compositor |
| chromium is found as node finds it: `STUDIO_CHROMIUM`, then the usual paths | the image names it; a device keeps the browser's own export |
| chromium is started by `k7-picture`, not by `chromiumoxide`, with the account's environment alone, then reached over its DevTools socket | the front holds the install's token and the hub's keys; `chromiumoxide`'s launch adds to what chromium inherits and cannot clear it |
| the page posts the film back as base64 text in 3 MiB chunks (`result?type=…&last=1`), and the server appends each to a part file | the request gate pauses every request and chromium puts the body in the DevTools event: a binary body carries lone surrogates that no JSON parser accepts, so the event is lost and the render hangs, depending on the film's bytes |
| the DevTools protocol goes over a loopback port chromium draws (`--remote-debugging-port=0`), not a pipe | `chromiumoxide` speaks WebSocket only. Any process of the machine can reach that port while a render runs: accepted, since on the server every process is the image's own and the render page holds no secret |


## The code root

| host | where it is | how the front hears it |
|---|---|---|
| a checkout (dev, a test bed) | the checkout itself | found above the front's working directory or executable |
| the desktop app | `resources/code`, staged by `apps/k7-daw-desktop/scripts/stage.mjs` | `K7_CODE_ROOT` in the front's `Config.env` (`backend.rs`) |
| Android | `code/` in `backend.zip`, staged by the `stageCodeRoot` task | `K7_CODE_ROOT` from `BackendHost.kt` |

| choice | why |
|---|---|
| one script stages it everywhere, `apps/k7-daw-backend/scripts/stageCodeRoot.mjs`: `apps/k7-daw-frontend/packages/song/meta`, `apps/k7-daw-frontend/packages/song/samples/video/sample.mp4`, `apps/k7-daw-frontend/packages/commands/commands.yaml`, `demos/`, at their repo paths (some 700 KB, 99 files) | the pieces read these files at their repo paths under the root; a test (`switch.rs`) names what a code root must hold |
| picture render's page is not in it: `K7_RENDER_PAGE` names it (the image), a checkout builds it beside the crate | a build of its own (esbuild), and only the server renders pictures |
| a `K7_CODE_ROOT` that lacks them is said in the log and ignored | a wrong folder must not run every compile against missing files |
