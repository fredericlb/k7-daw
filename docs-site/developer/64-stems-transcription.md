# Stem separation and transcription

Two long-running jobs that feed the DJ side: cutting a record or a clip into stems, and turning a video's speech into timed text. Both are slow, both run outside the browser, and both refuse loudly rather than half-succeed.

## Stems: the vocabulary

`packages/song-schema/src/stems.ts` is the one definition three packages share:

- `STEMS = ["drums", "bass", "other", "vocals"]` **in the order the model returns them**; the output tensor carries no names, so the order is the knowledge.
- `STEMS_DIR = "stems"`: outputs go in `stems/` beside the source, as `<stem>.wav` (`stemFile`). `INSTRUMENTAL` (`instrumental.wav`) sums every stem but vocals.
- `stemsComplete` requires all four: a deck plays them in place of the record.
- `STEM_PARAM = "Gain"`, `STEM_RANGE = [0, 2]`, `STEM_NEUTRAL = 1`: the desk knob drives the stem platter's own gain.
- Node names: `stemLane`, `stemPlatter`, `deckPlatter`, `stemControl`, `stemLaneOf`, `turntableMembers`, `TURNTABLE_REF`.

Tested by `packages/song-schema/src/stems.test.ts`.

## Stems: who runs the cut

A separation runs an ONNX model on an **engine's machine**, inside the native supervisor (`engine/k7-supervisor`); no Node process here runs the model. Servers only start, track and poll jobs.

| Path | Role |
|---|---|
| `packages/stem-separation/src/separateJobs.ts` | The seam: `SeparationRunner` (`start`, `job`), `setSeparationRunner`, `startSeparate`, `separateJob`, and `separationId`, a hash of (user, source, model, outputs) so a retry joins the job already running. |
| `apps/k7/backend-rs/crates/k7-front/src/routes/files/service/native/jobs/separate.rs` | The account's separation runner: sends the job over the bound machine's jobs channel. Finished jobs are kept 5 minutes for a last poll. |
| `packages/commands/src/runtime/members/jobs.ts` | `runColdStep` for verbs (`clip process`, `playlist stems`) polls the job until done; the separation chains come from the bundle's preferences. |
| `apps/k7/backend-rs/crates/k7-file-service/src/chains/cold_run.rs` | Resolve which model and outputs a chain runs. |
| `apps/k7/backend-rs/crates/k7-front/src/routes/files/service/native/verb_host/stems.rs` | The `playlist-stems` verb host: each record of a playlist is cut through the chain's steps and its stems are written in `stems/` beside it; one failure stays on its record while the loop goes on. |

"Already done" is always asked of the folder beside the record, never of a flag.

### Two ways a cut starts

1. **`playlist stems <playlist> [chain]`** (`packages/commands/src/verbs/playlist.ts`): resolves the chain against the open project (`resolveChain`, `@studio/audio-passes/processChain`), refuses one with no cold step, then hands the playlist's records to the host (`playlistStems`), which finds each one in the workspace, skips those whose `stems/` already holds audio, and cuts the rest sequentially. A per-record failure is a log line; the loop continues. A set is prepared this way beforehand: the desk separates nothing itself.
2. **The playlists panel's "Separate the stems"** (`apps/k7/frontend/src/components/PlaylistPane.tsx`, `StemsBar.tsx`): the same verb, run through the shell so it can be watched and stopped.

The desk only reads the result: a record with its four stems beside it plays on the stem platters (`SetEntry.stems`), an unprepared one plays on the deck's own lane.

### Machine preferences

`system stems`, `system instrumental`, `system onnx`, `system cue` (`packages/commands/src/verbs/system.ts`) write the machine's preferences. `system stems` checks the chain against the open project when there is one; `system instrumental` additionally refuses a chain without an `instrumental` output. `system onnx` defaults to `cpu` alone: a wrong accelerated provider hangs the job rather than falling back. Tests: `packages/commands/src/systemOnnx.test.ts`.

## Transcription

The `transcription` plugin (`packages/k7-plugins/src/transcription/`) is k7's one builtin **server** plugin, installed by the verbs runtime through `packages/k7-plugins/src/server.ts` (`installBuiltinPlugins`). It contributes one command, `track transcribe`, hosted in the core `track` family.

| Path | Role |
|---|---|
| `index.ts` | The `definePlugin` declaration: sid `track.transcribe`, args `track_in` (completed from `pictureLanes`) and `track_out`, `menu: ["track"]`. |
| `transcribeTrack.ts` | The handler, acting through `CommandHost` like any core verb. |
| `transcribe.ts` | `extractAudio` (ffmpeg, mono 16 kHz), `transcribeAudio` (the whisper CLIs), `parseSrt`, `srtOf`. |

Choices:

- **Everything is checked, and whisper runs, before a line is written.** A refused transcription writes nothing; an existing picture lane `track_out` has its own caption placement replaced, and the command journal makes the whole thing one undo.
- **Subtitles, not a field.** The transcript is `clips/<track_out>.srt`, a caption clip naming it, placed on the picture lane `track_out` at the shot's `start`, `length` and `from`: a caption clip is what the timeline, the viewport and a render all draw.
- **The first shot of the lane** is the source, timed in its own seconds. Re-timing segments onto a lane that cuts between videos is not done.
- **SRT as the stored text**, so the file is a subtitle file as is. `parseSrt` accepts both `,` and `.` before milliseconds because the two CLIs differ.
- **Whichever whisper the machine has.** `CLIS` tries `whisper` (openai-whisper) then `whisper-cli` (whisper.cpp, which needs `STUDIO_WHISPER_MODEL`). A missing binary (ENOENT) is skipped; one that ran and failed is the error, with no fallback, to avoid a second model download. No image in this repository installs whisper, so it runs only where somebody installed it beside the file service.

Tests: `packages/k7-plugins/src/transcription/transcribe.test.ts` (SRT round trip, dialects) and `trackTranscribe.test.ts` (the verb's refusals and writes). `packages/k7-plugins/src/composition.test.ts` checks that both composition lists install into the registries cleanly and stay independent lists.
