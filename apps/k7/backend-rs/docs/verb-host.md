# The host members' routes (step 3f)

`/verb-host/…`: what a verb run in the browser's Worker asks the server for when it needs the machine (a render, a probe, a
separation). The code is
`crates/k7-file-service/src/verb_host/` (the job table, the gates), the front's `native/verb_host/` (the table of routes, the
frame) and the Worker's `packages/commands/src/runtime/members/`. The routes belong to the `journal` piece and have no fallback.

## Choices

| choice | why |
|---|---|
| a route whose driver or piece is off answers 503 with `code: "off"` | a member must refuse by name where its piece is off |
| the frame judges the shape, the open project and the weight; the member judges its own paths through the frame's gate | the frame cannot know which field of a body names a file |
| a path is read only under a virtual root the person reads, judged lexically and again through the disk's links | a link out of a root would otherwise read anything the server can |
| a refusal repeats what was sent and names no folder of the server | the browser composes every body; a real path in an answer would leak the layout |
| a file is written only through `.job-out/<job>/<out>`, a name of one segment | a verb that fails after a job lands nothing, and the change set journals what does land |
| the job table is handed `now` and reads no clock | the heartbeat and the forgetting are tests on a turned clock, not sleeps |
| a job nobody polls for 60 s is stopped, except `prepare` and `picture`; the poll is the heartbeat | a long verb lands only while its page stays open; those two outlive it |
| a finished job's record is kept 10 minutes | a slow poll must still find its answer |
| an id is 128 random bits and belongs to one account; another's is a 404 | an id is no capability across accounts |
| an account runs at most 8 jobs; the ninth is a 503 `busy` | one page cannot fill the machine |
| the body limit is the route's (64 KiB; a doc or a scene 8 MiB), applied while the body is read | a limit judged after parsing has already paid for the body |
| the Worker marks the run `hostCalled` before the request | a run that reached the server is no pure function of the Vfs, so a 409 afterwards is said, never rerun |
| a job's file enters the Worker's Vfs as an adopt; the copy of an adopted file is another adopt, and one left under `/tmp` is not posted | a `copy` would name a file the server does not hold yet, and a render's scratch copy is nobody's |
| `no-ffmpeg` throws the class a picture verb catches; every other code is a `HostError` carrying it | the verbs' own catches already word each failure |
| closing the runtime stops the polls; the heartbeat then stops the jobs | a closed page must not leave a render running for nobody |
| a render's doc is parsed as the song crate's `ProjectDoc` and every path field it names is mapped and confined in one walk (`verb_host/docpaths.rs`: clips, frozen material, timeline lanes, plugin file fields, banks, rack sounds, the record folder) | a field the wire type does not name cannot carry a path; the walk lists the fields of the share's `rehome`, so a new path field is added to both |
| a relative path in a doc resolves under the render's `base` alone, which is itself judged; with no base it is a 403 | the engine would otherwise resolve it against its own working folder |
| the gate answers the canonical path it checked, never the lexical one; a path the disk cannot resolve (a dangling link, no access) is a 403, and a path not made yet is judged by its nearest folder | a link swapped between the check and the open must not redirect the read |
| `/changes` adopts only the output of a job of the same account that ended with its value | a running or stopped job's file is half written |
| a job holds its place in the account's bound until its driver returns, not until its record ends; a driver's hold ends the job if it never answered, and a job that did not end with its value leaves no folder | a page that goes quiet cannot out-run its drivers, and a failed render leaves no hundreds of megabytes |
| a job starts a sweep of `.job-out` folders a day old | an account that never lands another adopt would keep every unused render |
| the Worker sends the runtime's own project name as `at`, whatever the verb put there | a verb's `songPath` is the virtual folder, which no `at` check could match |
| a runtime's `open`, `run`, `complete` and `refresh` take turns | one Vfs and one models root serve a runtime, and overlapping runs would roll each other back |
| each separation is keyed by its job, so two asks of the same work are two jobs; each reads only its own folder | a joined record would name files in another job's folder |
| a playlist's record names are workspace-relative paths: a name with an empty, `.` or `..` segment, a backslash or a link out of the workspace is `absent` | the names come from a browser body |
| a poll that cannot reach the server (a lost connection, a 502, 503 or 504) is asked again for 30 seconds | a restart of the front must not end a verb whose job goes on |
| a probe is ffprobe's own reading (`k7-mediatek/src/video_probe.rs`, the arguments and the JSON to facts), run by the front; a machine without it answers 503 `no-ffmpeg` | the phone's reader without ffprobe is a decision of its own (3f-6); every coercion the JSON goes through is pinned by vectors the TypeScript recorded |
| a probe's failure repeats the browser's path, never the server's | ffprobe's own line names the file it opened |
| an extract writes `sound.wav` into the job's folder and a film with no sound is a finished job with no file | the sound is a deliverable the verb adopts, and a silent film is an answer, not a fault |

| a prepare answers `{started}` at once and works on after the page: ffmpeg writes `<copy>.part`, which is renamed over its destination only when it finished, and the clip's field lands after its copy (`edits::prepare::land_clip_copy`, under the journal's gate and the write lock, the revision bumped) | an interrupted run leaves nothing a compile would take for a finished copy, and a field never names a copy that is not there |
| the proxy is made from the clip's optimized copy when the file is there, else from the source | the optimized copy is also the faster read |
| a copy is written inside the open project, beside the name the clip file gives its source; a name with no extension is refused | a copy named by a clip file's text must not leave the project, nor land on the source itself |
| a source is prepared once at a time per account, by the tag `picture prepare <source>`; the account's background registry lists it when it has one | the same file encoded twice is minutes of the machine; the registry is what the process list shows |
| a transcription is the CLI's own SRT read and written again by the Rust half (`k7-mediatek/src/whisper.rs`), the audio decoded to 16 kHz mono by the WebAssembly decoder and by ffmpeg only for a container it cannot open | the bytes of a transcript are the TypeScript's, pinned by vectors; the CLI is whoever the machine has |
| a CLI that is not installed is stepped over; one that ran and failed is the error, said as it said it, and the other is not tried | retrying with another dialect would double a model download nobody asked for |
| a transcription's work folder is the job's own and is removed with the answer | the answer is text, no file is adopted |
| a picture is a job whose value is `{rel}`, the film's path from the project: the Worker's scene is posted whole and the front writes the film into `exports/` itself (one render at a time per account, a second ask is 409) | a film is a deliverable of the project folder, not a change set, and the person closes the page long before a render ends |
| a scene's `media/` URLs carry virtual paths, each judged by the person's read gate; one outside every root is left out and the page's fetch of it is a 404 | the scene is the browser's text, and the render page reaches only what the person could read |
| the sound is the native mixdown of the open project as it stands on disk, asked only when `audio` is true | the verb asked for it only when the project wires the sound, and a silent film needs no machine |
| the account's own members (engine, plugins, devices, onnx, library) read no path: they need no open project and hold no share of the call gate | a rescan waits a minute and the page asks them with no project open |
| a Windows name that is a device or a stream (`NUL`, `COM1.wav`, `a.wav:x`, a trailing dot or space) is refused by the read gate on Windows | opening one hangs a job or hides data from the tree |
| a canonical path is joined only in the `/…` or `X:/…` spelling; a UNC or device form is refused | the path library collapses `//host/share` into the current drive |
| `onnx` with no engine or no machine is `null` to the verb, which words it | the verb's own refusal is the sentence the person knows |
