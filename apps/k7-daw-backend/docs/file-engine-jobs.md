# The engine's jobs in native Rust

What the account asks of an engine beyond playing its project (auto-freeze, the renderers, separation, the plugin queries, the preview source, take arming), held by the
front over the native engine link (`file-engine.md`). This page holds the choices and what forced them; the numbered rows
(2300 to 2399) say where each is held.

## What the jobs cost

Measured on the Mac (Apple M5 Pro, a load average of 8 to 26 from other agents), with the engine binary of a bed
build started alone, silent, in a scratch home, its environment cleared:

| measure | value |
|---|---|
| an offline render (`k7-runtime --render`), a process that exits at once (nothing audible) | 0.52 s: the fixed cost of a render is the engine process starting |
| a render of 4 s of one audio clip / of 62 s of the same clip | 1.0 s / 1.87 s: about 22 ms per second of audio past the start |
| a render of the golden `synth-demo` (6 s, three built-in synths) / `surge-synth` (4 s, Surge XT) | 1.2 to 1.8 s / 1.03 s |
| the same, with `--stems` (three stems and the mix) | 2.87 s |
| two renders of one audio-clip document | the `data` chunk identical; the files differ in their `bext` chunk, which carries the render's date |
| two renders of one synth document | every sample differs: the synths' oscillators start at a free-running phase |
| `list-plugin-params` for an installed VST3, first / again | 77 ms / 33 ms (312 parameters); 211 ms / 117 to 123 ms (567 parameters) |
| `list-plugin-params` for a built-in (`format: internal`) | 0.6 to 4 ms, answered `found: false` |
| `snapshot-plugin-base` with no orphan window open | 0.2 to 0.5 ms, answered `ok: false` |
| `list-sessions` round trip, for scale | 0.09 to 0.17 ms |
| the take hole (`withTakeHole`) on a lane of 1 / 100 / 2,000 clips | 0.2 µs / 8 to 15 µs / 120 to 240 µs |

| what was read | what it decides |
|---|---|
| a render is a process start and a bounce, half a second at least, minutes for a long share with stems | a render never runs under a lock or awaited by anything a poll waits on; its child dies with its task |
| a plugin query is the engine loading the plugin: tens to hundreds of milliseconds, 20 s allowed | the wait is a timer on the front, never a turn behind a verb |
| the take hole is microseconds | the hole stays where the link applies it, at the send; the jobs hold only the arm's memory |
| a synth's render is never the same twice; an audio clip's is, sample for sample | tests compare a render's `data` chunk on an audio-clip fixture only, and a synth's by format, length and level; never a file's bytes |

## Where the line runs

| choice | why |
|---|---|
| the front owns the plugin queries, the audition, the thaw and the automatic freezer's timer, the renderer and the separations' registry | each waits on the engine, a child process or a timer, and must not wait behind a verb |
| the crate holds the freezer as a machine without I/O (a tick, a compile's freezes, a render's outcome in; compile, render, reload and log out) and the pure resolutions (a plugin's pair, an audition's file and window) | the TypeScript's 3 s pass and 20 s idle thaw are tested on a turned clock; the vectors are the TypeScript's own traces |
| a render goes to this machine's engine when one is found, else to the bound machine, else to the server's render machine; a machine that fails hands it to the next, and a base folder never goes to a machine | the choice is made from memory when the driver is made, never by a probe on the request's path; a base folder names a path on this disk, which a machine would read as another's |
| on Unix a render's child leads its process group, and a kill before its exit (the bound, a request that went away) takes the group | a wedged plugin's helper outlived every killed render; Windows keeps the engine alone until a Job Object is worth its code |
| the freezer is the crate's machine run by the front: a 3 s timer over the link's served projects, each effect carried out in a task (a compile on its own big-stack thread, a render by the driver, a reload by the session), a share's `renderAll` queued in the machine behind a pass | the machine decides what the TypeScript's `AutoFreezer` decided, so its traces are the referee; no lock is held across a compile or a render, and a panicking compile fails one pass, not the front |
| a freeze's render goes to the bound machine when there is one, else to this machine's engine, and lands by a rename of a name beside it | a freeze's name is a fingerprint, never rendered again once it exists; a copy killed half way would play truncated for a month |
| the freezer's compile reads a project through the account's `Docs` and the failed jsdsp builds the project's last interactive ask carried; a pass failing on a jsdsp build logs nothing, since the interactive compile reports it | a failure logged every 3 s would drown the log |
| the thawed modules, the take's arm and the separations live in the account's `Held` | they outlive any one request: a thawed plugin must not freeze back under a knob, an armed take must not land on the lane, a cut's progress must not be forgotten |

## The queries, the audition and the thaw (2305, 2306, 2340 to 2345, 2362)

| choice | why |
|---|---|
| the three plugin queries are account routes: no workspace header, no owner check, as the TypeScript mounts them before both | a plugin is a fact of the machine, not of a workspace |
| a query waits on the link, not on a session: one list of waiters per link, settled by the frames of any session's socket, each matched on the id (parameters, window) or the path (a base) it was asked for | the engine answers the asking connection, and another surface may be asking at once for another plugin |
| a waiter is registered before its frame is queued and leaves the list on its answer, its limit or its request dropped; one frame settles every waiter on its type and key | an instant answer would otherwise be lost, and a client that left must leave nothing behind |
| `Jobs::ask_until` gives an ask a `gone` future, and `Driver::render_run_until` has no default, so a driver cannot ignore it; a `render.run` whose asker is gone is dropped, which kills a local render's child (a machine's render is abandoned: its HTTP ask is hung up and the remote engine finishes), and makes no reply; a finished render wins over `gone` (`biased`) | a request that went away (an `/export` hung up) must not leave this machine's engine rendering for a folder nobody reads |
| silence is read per query: parameters 503, window 504, base 503; a window the engine refused 502, a base it did not write 409; the limits are 20 s, 30 s and 5 s | the TypeScript's own readings, which the editor's messages depend on |
| the plugin's pair comes from the catalogue copy under the studio home, read only for a `ref`; a repeated query key is its values joined by commas | the copy is what the scan gate writes; Express hands an array to `String` that way |
| an audition is resolved on a blocking thread, the compiled document asked for once and only when a clip id needs it; its frame goes into the workspace's session after any document | an audition of a file names no clip, and a click must never play before the document it depends on |
| a snapshot's folder is made under the account's write lock; the engine's answer is waited for outside the lock | a folder made is a write, a wait of five seconds is not |
| a snapshot that the engine never answers has still made its folder | the TypeScript made it before it asked |
| a thaw changes the account's thawed list, then reloads the open project through its session; with no open project it answers ok and changes nothing | the list is what the next compile reads, and the TypeScript's `thawAutoFreeze` returned early the same way |
| the front logs `[files] native jobs <account>: renders= freezes= queries= previews= thaws= separations= panics=` every 30 s, and only when a count moved since the last line; the judge sums the accounts' last lines | a run that queried nothing must not read green, and an idle front stays quiet |
| APPROVED (review B3, decision 1): the freezer compiles with the failed jsdsp builds the last interactive ask of that project carried, whether it is the link's own compile (`RealHost::engine_doc`) or a docs ask | without them a project with a failed build fails every pass and logs it every 3 s; the user approved it |
| `bedCounters.mjs` reads the jobs lines summed over the accounts: floors `previews` 6, `queries` 4, `thaws` 3 and `renders` 7, `panics` 0; `freezes` and `separations` have none; a run without the conformance suite is not judged, a filtered one prints the floors as information, and a panic fails either way | the conformance suite reaches no VST project and cuts nothing through a verb, so the live harness is their referee; the counts are exact ones from a filtered Mac run and a floor is raised after the bed, never lowered |

## The replays and the live harness (2360, 2361)

| scenario | file | what is compared |
|---|---|---|
| a render, a freezer pass, `renderAll` and a cut | the crate's and the front's own tests (`tests_render`, `tests_freezer*`, `tests_separate`, `tests_driver`) | no recorded trace reaches them alone |
| the local render on the sine fixture | `jobs/tests_jobs_live.rs`, `#[ignore]`, `K7_LIVE_ENGINE_BIN` | the `data` chunks of the native and the TypeScript renderer |

The jobs' replays are fixed scenarios with no seeded sequence: each step waits on a landmark, and a failure prints its scenario's name. The real engine is the live harness's alone; the automatic freeze of a VST project is not in it yet.

## On disk and on the wire

The engine's frames and the jobs path's requests are unchanged (`engine/k7-runtime/docs/protocol.md`,
`packages/engine-tunnel/docs/jobs.md`). An automatic render keeps its name (`<studio home>/.renders/<module>-<hash>.wav`,
the fingerprint the `song` crate computes) and is written beside itself under a temporary name, then renamed: a
render killed mid-copy must never be read as made, since its name is never rendered again.

## What renders, background work and processes decide

| choice | why |
|---|---|
| a render is a separate one-shot process, never a verb of the live engine's socket | the live engine holds the audio device and a second engine beside it wedges an ASIO driver, after which every engine start blocks with no output; the safety rests on a process boundary |
| a render goes to this machine's engine binary, else the bound engine's machine, else the server's render machine, and a machine that fails hands it on | a hosted k7 keeps no engine binary; a musician's supervisor may predate the job's `outDir` form |
| a remote render lands under a writable root (`.renders/jobs/<id>`) and is copied into the caller's folder, then the entry goes; a base folder is refused on a machine | the cache and a scratch folder are on different volumes; a base names a folder on a disk the server cannot see |
| a render owns its scratch folder until it succeeds and removes it on every failure | a wedged machine renders repeatedly and each attempt costs a song of disk |
| no engine to render with, and exit 0 with nothing written, are errors with a sentence of their own | a document whose every track was filtered out is a silent failure, not an empty render |
| `picture render` draws with the browser's own compositor in a headless Chromium, `webm` by default, from a one-off server on the loopback under a random path | a second renderer is a second copy of the shaders and captions and drifts; WebCodecs and WebGL2 exist in no Node runtime; VP9 is in every build and H.264 depends on the distribution |
| a camera lane draws nothing in a render, and an unwired RENDER sound input renders silent | a camera has no frame at second t to pull |
| a verb's background work runs one job per tag, so running it twice joins the first; its result is what it writes, and one line says how it ended | a transcode takes minutes and a held request would freeze the command bar; the clip field naming the copy moves the revision every view follows |
| the processes list holds what is running (background jobs and any command still running after a second), is forgotten a moment after it ends, and rides the `/workspace` poll | what is running is in no file; a result is a file, never a process |
| the separation a `clip process` runs goes through the job registry, whose id hashes the work, and the registry keeps a finished job five minutes | a retry or a second tab joins the run in flight instead of starting another hour of arithmetic; a job forgotten between two polls is said as itself |

## The vectors (2303, 2301)

`tests/vectors/jobs.json` is frozen: no generator is kept. It holds the TypeScript's own answers: `resolvePlugin` over a scan of 40 plugins, `resolvePreview` through a real `Workspace`
over seven repository projects and fourteen made ones (windows in seconds and as a timecode, a placement id, an escaping and
an absolute source, an NFD name, a library's `samples/`, `irs/` and `modules/`), the `AutoFreezer`'s traces on a clock the
script turns, `separationId`, `parseManifest` and `moveInto`. A path in an answer is spelled `{base}`. The hostile sizes are
4 KiB here; the crate's own hostile tests make the 64 KiB ones. The other-case name (`Beat.WAV` for `Beat.wav`) is not a
vector: the answer follows the volume, so a test of it asks the host. Song's thawed memory is `thawed.ts` (2301).
