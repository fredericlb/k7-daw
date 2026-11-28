# DJ side: tests and measurements

The desk's design splits every decision that can be wrong into a pure module so it can be tested without React or an engine. No test mounts a desk component; a visual or interaction change has to be checked in the running app.

## Running the tests

Each package runs vitest on its own:

```bash
npm run test --workspace @studio/k7-plugins
cargo test --manifest-path apps/k7-daw-wasm/song-schema/Cargo.toml
cargo test --manifest-path apps/k7-daw-wasm/verbs/Cargo.toml
```

From the root, `npm run test` runs `scripts/workspaces.mjs test`, which runs the `test` script of every workspace that has one (`--jobs N` to run several at once). Remember the repo's lint rules for any `.ts`/`.tsx` you touch (`npm run lint`).

## What is covered

### `apps/k7-daw-frontend/packages/k7-plugins/src/turntables/`

| Test | Holds |
|---|---|
| `mixBindings.test.ts` | The lookups against the engine's live state: the control behind a module's param, what a control shows, whether the engine has caught up, the cue's own pair. |
| `cue.test.ts` | Eight cue colours, all distinct. |
| `setList.test.ts` | Search folding and sorting, drag lookup, the words a playlist's trouble is told in. |
| `prewarm.test.ts` | Sequential walk, failures counted, early stop. |
| `service.test.ts` | Cache-relative and playable paths, Windows and POSIX roots. |
| `waveform.test.ts`, `shuttle.test.ts`, `dial.test.ts` | What the waveform, the shuttle and the record draw. |
| `deskLayout.test.ts`, `views/crossfadeReach.test.ts` | What fits at a given height; the crossfader reaching the cue rows. |
| `views/ChannelStrip.test.tsx` | Knob geometry (`angleOf`, `arcPath`) and icon tables — functions only. |

`views/ChannelStrip.stories.tsx` is a story for looking at the strip, not a test.

### `apps/k7-daw-wasm/turntables/tests/`

| Test | Holds |
|---|---|
| `rules.rs` | `beat_phase` (circular mean, Rayleigh confidence), `align_seek` (shorter way round, never before 0), a corrected tempo, loops (snap, length rounding, edges clamped into the record, **write order** of `Roll`/`LoopStart`/`LoopEnd`), the crossfade law, sync, keylock, the scratch pace, when a tempo move is worth a document. |
| `deck_doc.rs` | `with_decks` and `deck_doc`: platter params, stems versus record lanes, `Seek` never re-sent, the stand-in document, which deck the click follows and the click's bar. |
| `bindings.rs` | Ids unique and per deck, ranges equal the modules' own, required controls, binding resolution, which track a fader stretches, what a control sends, offline tempo. |
| `set_list.rs` | Resolving a playlist against the workspace's files, the three failure states, and the scaffold's control list held to the original. |

### Elsewhere

| Test | Holds |
|---|---|
| `apps/k7-daw-wasm/k7-wasm/src/tests_text_desk.rs` | Parsing and the text edits (append, remove, move), every refused shape. |
| `apps/k7-daw-backend/packages/k7-file-service/tests/all/listings_workspace.rs` | Which files of `_references/` a listing offers. |
| `apps/k7-daw-wasm/song-schema/src/stems.rs` | Stem names and node naming. |
| `apps/k7-daw-wasm/k7-wasm/src/tests_text_desk.rs` | The cue sheet: reading, and writing markers into the text. |
| `apps/k7-daw-wasm/song-edit/tests/replay_lane/main.rs` | The mix and live scaffolds compile and wire what they say. |
| `apps/k7-daw-wasm/song/tests/all/assemblies.rs` | The deck assembly; migrations giving old sets a new set's files. |
| `apps/k7-daw-wasm/verbs/tests/playlists.rs`, `playlistStems.test.ts`, `systemOnnx.test.ts` | The verbs. |
| `apps/k7-daw-wasm/verbs/tests/transcribe.rs` | `track transcribe`'s refusals and writes. |
| The tests in `apps/k7-daw-frontend/packages/audio-analysis/src/` | Decode through a stand-in engine, envelope arithmetic on a synthetic click track, the sidecar. |

## Measuring against an engine: `put-on`

`apps/k7-daw-wasm/put-on/src/main.rs` measures what putting a record on an already loaded deck costs: it writes test WAVs, sends the documents `with_decks` builds to a running engine, and times the `applied` answer.

```bash
cargo run --manifest-path apps/k7-daw-wasm/put-on/Cargo.toml -- [ws://127.0.0.1:8765] [--minutes=2] [--runs=6]
cargo run --manifest-path apps/k7-daw-wasm/put-on/Cargo.toml -- --hold=30 [--key]
```

`--hold` loads two separated decks, starts them and holds, so the CPU of eight running platters (with or without keylock, `--key`) can be read from outside. Never run it against an engine you are building at the same time.
