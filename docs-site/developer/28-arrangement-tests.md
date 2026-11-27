# Testing the arrangement code

Every test here is vitest and every one is pure: nothing mounts a React component and nothing starts
an engine. That constraint shaped the code — logic that decides what lands in a file is pulled out of
the components into modules a test can import — and it is also the gap: a green run says nothing
about a gesture's feel or a drawing. Exercise those in the running app.

## Running them

```bash
npm test                                   # every workspace, from the repo root
cd apps/k7-daw-frontend && npx vitest run      # the frontend's pure modules
cargo test --manifest-path apps/k7-daw-wasm/verbs/Cargo.toml     # the verbs
cd apps/k7-daw-wasm/song-edit && npx vitest run    # the k7x writers
npx vitest run src/clipLoop.test.ts       # one file, from inside its package
cargo test --manifest-path apps/k7-daw-wasm/gesture/Cargo.toml   # the gestures' arithmetic
```

Lint must pass on any TypeScript you touch: `npm run lint` (biome). `npm run lint:docs` refuses a
source comment or a Markdown page naming a file that is not there.

## What covers what

### Frontend (`apps/k7-daw-frontend/src/`)

| Test | Pins |
|---|---|
| `timelineMode.test.ts` | The derived mode, and that a pending move does not own the keys. |
| `clipboard.test.ts` | Copy by start, relative coordinates, paste offsets, dropped rows, carried placement fields. |
| `pendingEdits.test.ts` | What each one-shot line predicts on the lanes. |
| `apps/k7-daw-wasm/song-edit/tests/buffer_intake.rs`, `fileDrop.test.ts` | Kind by extension, clip id, lane walking; accepting a file drag. |
| `rows.test.ts` | The row model. |
| `historyMarks.test.ts` | Which lanes, clips and nodes a journal step lights. |
| `keys.test.ts`, `commandFill.test.ts`, `contextMenu.test.ts` | Key resolution, caret answers for a command's form, menus derived from the catalogue. |
| `clipLoop.test.ts`, `clipInstances.test.ts`, `clipEnvelope.test.ts`, `clipOps.test.ts` | Loop marks, repeated material, the activity envelope, process lines. |
| `apps/k7-daw-wasm/song-edit/tests/clipboard_draft.rs` | Curve drafts and the clipboard. |
| `takes.test.ts`, `midiTake.test.ts`, `midiIn.test.ts`, `webmidi.test.ts`, `laneInput.test.ts`, `transportPanic.test.ts` | Comp lines and spans, the take counter, MIDI take trimming, device matching, inputs, the double-stop panic. |
| `exportArgs.test.ts`, `gainTrim.test.ts`, `mixerStrips.test.ts`, `apps/k7-daw-frontend/src/mastering/controls.test.ts` | Export syntax, the Fix trim, strip order, the mastering control list. |

### Gestures (`apps/k7-daw-wasm/gesture/src/`)

The arithmetic of the page's gestures is Rust, and its tests are Rust `#[test]`s beside each module:
`snap.rs` (grid, relative and free snapping, the song-start floor), `roll_drag.rs` and `roll_fit.rs`
(note gestures, what the roll opens on), `grid_caret.rs`, `bar_window.rs`, `clip_playhead.rs`,
`clip_warp.rs`, `take_counter.rs`, `video_drag.rs` (picture drags in seconds), `tools.rs` (each tool's
line and the refusals at an event's head and tail), `ticks.rs` and `ruler.rs` (the ladders and labels),
`pan_zoom.rs`, `edge_drag.rs` and `layout.rs`. `@studio/k7-wasm`'s `gesture.test.ts` holds how each
call crosses the ABI and what a pointer move costs.

### Verbs (`apps/k7-daw-wasm/verbs/src/`)

`eventsMove.test.ts`, `eventsRemove.test.ts`, `eventStatus.test.ts`, `eventLoop.test.ts`,
`eventGain.test.ts`, `eventTakes.test.ts`, `eventProcess.test.ts`, `crossfade.test.ts`,
`curvePoint.test.ts`, `curveSet.test.ts`, `trackAutomation.test.ts`, `notesRegion.test.ts`,
`noteVelocity.test.ts`, `sequenceDouble.test.ts`, `sequenceRecord.test.ts`, `trackMuteSolo.test.ts`,
`trackHighlight.test.ts`, `trackOrder.test.ts`, `trackGroup.test.ts`, `trackInput.test.ts`,
`projectMaster.test.ts`, `projectConsole.test.ts`, `refusal.test.ts`, and the crate's `palette.rs`, which
holds the catalogue itself to its rules (both languages present, readable sentences at most 60
characters, known completion providers). `apps/k7-daw-wasm/verbs/tests/support/mod.rs` is the fake
`CommandHost` they run against.

### Writers (`apps/k7-daw-wasm/song-edit/src/`)

`moveEvent.test.ts`, `clipResize.test.ts`, `eventWindow.test.ts`, `eventFade.test.ts`,
`eventLoop.test.ts`, `eventWarp.test.ts`, `loopWindow.test.ts`, `crossfade.test.ts`,
`materialCut.test.ts`, `insertTime.test.ts`, `onsets.test.ts`, `sequenceDoubleLoops.test.ts`,
`addTrackPlacement.test.ts`, `trackMix.test.ts`. The house style is to
assert that **exactly one line** of the file changed, as `moveEvent.test.ts` does — that is the
property the whole write path is built to keep.

### Other packages

| Package | Tests |
|---|---|
| `apps/k7-daw-frontend/packages/ui-timeline/src/` | `panZoom.test.ts` (which wheel zooms, the factor, the ceiling), `ticks.test.ts`, `drag.test.ts` |
| `apps/k7-daw-frontend/packages/ui-waveform/src/` | `ticks.test.ts`, `spectrum.test.ts`, `spectrumColour.test.ts` |
| `apps/k7-daw-frontend/packages/audio-probe/src/` | `trimGate.test.ts`, `key.test.ts` |
| `apps/k7-daw-engine/k7-runtime/scripts/lib/take-render/` | `recordTake.test.ts`, `materialRender.test.ts` |
| `apps/k7-daw-frontend/packages/audio-passes/src/` | `processChain.test.ts`, `peakPass.test.ts`, `coldPass.test.ts` |
| `apps/k7-daw-backend/packages/k7-import/tests/` | `import.rs`, `golden.rs` |
| `apps/k7-daw-frontend/packages/engine-browser/src/` | `meter.test.ts` (meter fill, clip latch, peak hold) |
| `apps/k7-daw-backend/packages/k7-file-service/` | the journal, the arm, the renders and the change set, as Rust tests beside their sources and in `tests/` |

## Not covered

No test mounts `TimelineTab`, `AudioClipPanel`, `PianoRoll`, `MasterMeter`, `MasteringTab` or
`HistoryPanel`; `apps/k7-daw-frontend/src/hooks/useShare.ts`, `apps/k7-daw-frontend/src/hooks/useTrackControls.ts` and the `/export` route have no test
either. When a change touches one of them, move the decision you are changing into a pure module
first and test it there.

## See also

- [Extending: a new gesture or verb](./29-extending-arrangement.md)
