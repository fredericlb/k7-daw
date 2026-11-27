# Clip and note editors, and curves

A double-clicked event opens in the bottom panel (`openEvent` in `apps/k7-daw-frontend/src/App.tsx`),
drawn by `apps/k7-daw-frontend/src/components/ClipView.tsx`, which picks the audio panel or one of the
three note views. The editors follow the timeline's rules: every write is a command line, the view
state is the browser's, and the arithmetic sits in pure modules beside the components.

## The audio clip panel

`apps/k7-daw-frontend/src/components/AudioClipPanel.tsx` draws one event over its file.

**The picture.** The file's envelope is decoded and cached once per source by `apps/k7-daw-frontend/packages/audio-analysis`, in a
placement-independent `.cache` sidecar; `apps/k7-daw-backend/packages/k7-file-service/src/media/region.rs` slices it to the
region a clip plays and derives the bar/beat grid from the song's tempo and the clip's warp map
(`beat_at_source` / `source_at_beat` in `apps/k7-daw-wasm/song-schema/src/warp.rs`, asked through `@studio/k7-wasm/schema`). The file service
serves it (`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/media/waveform.rs`). `apps/k7-daw-frontend/packages/ui-waveform/src/Waveform.tsx`
draws it in the `spectrum` look (`spectrum.ts`): peak as height, the low/mid/high balance as colour —
the same painter the DJ desk uses, so there is one waveform in the product.

`apps/k7-daw-wasm/gesture/src/bar_window.rs` decides how many bars of the file are shown (twelve by default; the wheel zooms) and
where; `apps/k7-daw-wasm/gesture/src/clip_playhead.rs` maps the song's playhead to a second of the file, and is null outside the
event so the panel never shows a playhead that is not playing this event.

**Modes are the tool.** `apps/k7-daw-frontend/src/waveMode.ts` lists `standard`, `onsets`, `warp`,
`takes`. Unlike the timeline there is no separate tool axis: the marks you can touch are the marks
you can see. `ui wave <mode>` sets it; the select in the header dispatches that command rather than
setting state, so the menu, the key and the palette stay one path.

| Mode | Gesture → line |
|---|---|
| standard | Window edge drag → `event window t at from to` (seconds), or `event loop points …` on a looped event; a comped piece's edges → `event comp` lines (`pieceEdgeLines` in `takes.ts`) |
| onsets | Click → `clip onset add`; click a pin → `clip onset remove`; drag → `clip onset move` |
| warp | Double-click → `clip warp clip src beat`; handle drag → `clip warp` (same verb); right-click → `clip warp remove` via the `warp-point` menu |

`apps/k7-daw-wasm/gesture/src/clip_warp.rs` converts a display second into a **file second** and a **beat**.
The picture is warped, so the inversion has to walk the piecewise map, not divide by the overall
ratio — getting it wrong puts a stretch point somewhere the drawing never showed, and the redrawn
picture is "right about the wrong point". Its tests pin that.

Onset corrections are stored as a difference from the detector (`apps/k7-daw-wasm/song-edit/src/onsets.rs`),
in seconds, so re-detecting at another sensitivity keeps them; a time within 25 ms of an existing
onset means that onset.

**Header controls** map to verbs: the gain slider (`EventGainSlider.tsx`, ±6 dB, double-click 0) to
`event gain` (`apps/k7-daw-wasm/song-edit/src/event_gain.rs` rounds to a tenth and deletes at 0); the loop
toggle and the ruler brace (`LoopBrace.tsx`) to `event loop` / `event loop points`
(`apps/k7-daw-wasm/song-edit/src/event_loop.rs`: a looped event keeps its length and its pass count is
recomputed); the trim gate to `event trim`, previewed in the browser with the same gate maths the
verb uses (`apps/k7-daw-frontend/packages/audio-probe/src/trimGate.ts`); the operations menu to `clip process`
(`apps/k7-daw-frontend/src/clipOps.ts` builds the line).

## Windows live on the event

A material file describes the bytes; a placement describes the use. `from`/`to` are therefore on the
**event**, and `event window`, `event trim` and `event resize` narrow only the event they address,
clamped inside whatever the material file still states. `apps/k7-daw-wasm/song-edit/src/clip_resize.rs`
turns "span these two beats" into the fields the model has (window and loop count; a placement has
no length).

## Note views

A MIDI clip is edited in the piano roll alone.

**Piano roll** — `apps/k7-daw-frontend/src/components/PianoRoll.tsx` draws; `apps/k7-daw-frontend/src/hooks/useRollGestures.ts` turns pointers into
gestures; `apps/k7-daw-wasm/gesture/src/roll_drag.rs` holds their arithmetic; `apps/k7-daw-wasm/gesture/src/roll_fit.rs` the key range and zoom. Modes come from
`rollMode.ts` (`select` default, `create`, `remove`, `takes`). A note has four grips decided on the
press — start edge, end edge, body, velocity line — and each commits one verb:

| Gesture | Line |
|---|---|
| Draw on a lane (create) | `sequence note new seq key start end` |
| Body or edge, time only | `sequence note timerange seq key start newStart newEnd` |
| Body, pitch changed | `sequence note move seq key start newKey newStart` |
| Velocity line | `sequence note velocity seq key start v` |
| Body of a note inside the rectangle | `sequence notes move seq [[key,start,newKey,newStart],…]` |
| Press in remove mode | `sequence note remove seq key start` |

A sequence FILE with no placement (`EditTarget` kind `seq`) draws `SeqRoll` in `ClipView.tsx`: the
same `PianoRoll`, given the clip roll's caret, `onCommand`, `modeRef`/`onModeChange` and
`onRemoveNote`, so its mode strip, `ui roll <mode>` and `x` work; it gets no `onSeek` and no event
loop, having no placement.

Notes are addressed by key and start, never by index. A command costs a round trip of a few hundred
milliseconds, so `rollPending.ts` paints the expected result immediately and drops it when the
reload lands or the command is refused.

`SeqDoubleButton.tsx` runs `sequence double`, whose writer then halves the loop count of every placement
(`apps/k7-daw-wasm/song-edit/src/sequence_loops.rs`) so the arrangement does not move. Repeat and skip are
`sequence note repeat|skip`, which the engine realises from the compiled document without expanding them.

**Inline grid** — `SeqGridFile.tsx` reads the sequence file and writes every edit back to it;
`SeqGrid.tsx` is the controlled grid, mounted from `@studio/ui-editor`. It backs `sequence edit roll`
and the editor popin's Roll tab, and writes the file directly rather than through `sequence note` verbs.

The roll shares the timeline's caret through `apps/k7-daw-frontend/src/hooks/useGridCaret.ts` and `gridCaret.ts`, and exposes
the `copy` / `pasteLine` / `deleteLine` contract to the shell, so `selection copy|paste|delete`
route to the grid holding a rectangle (`App.tsx`, `clipGridRef`). `deleteLine` answers two
lines: one note is `sequence note remove`, several are `sequence notes remove` over the
rectangle. Region verbs
(`sequence notes remove|transpose|quantize|velocity`) take two keys and a half-open beat span: the
rectangle exactly as drawn.

## Curves and automation

A curve is a file (`curves/<id>.k7x`, read by `apps/k7-daw-wasm/song/src/curves/file.rs`); a binding is a
module's `automations: [{param, curve}]`, keyed by param. Verbs:

- `curve set <name> <json>` — the whole curve, from the editor (`apps/k7-daw-wasm/verbs/src/verbs/curve.rs`).
- `curve point add|move|remove` — one point of a `custom` curve, addressed by its time. The last
  point cannot be removed: a custom curve with none does not load, and a curve file the loader
  refuses takes the whole project with it.
- `node automate <module> <param> <curve|none> [trigger]` — the binding.
- `track automation add|remove <track> <param>` — the same binding reached from the lane; `add`
  creates the curve flat at the knob's current value so binding changes nothing, and refuses when it
  cannot read that value.

**The editor.** `apps/k7-daw-frontend/src/components/CurveEditor.tsx` edits a draft (`apps/k7-daw-frontend/src/curveDraft.ts`)
and writes nothing until Save, which is one `curve set`. The draft is computed with
`@studio/k7-wasm`, the same Rust arithmetic the bake uses (`apps/k7-daw-wasm/song-schema/src/curve.rs`).
`CurvePlot.tsx` draws it and edits a custom curve's points; the kit `Knob` the ADSR knobs;
`CurveConditions.tsx` the when-it-runs rows (`start`, `end`, `between`, `event`).

**The timeline's automation rows** are `automation` rows in `rows.ts`, drawn by `TimelineTab` as a
polyline of the curve's points. They hold no gestures of their own: a right-click reports a
`curve-point` context with the time under the pointer snapped to a drawn tick, and
`contextMenu.ts` offers the three `curve point` verbs with the curve and time filled in.

## See also

- [The timeline component](./22-timeline-component.md)
- [MIDI, recording and takes](./24-recording-code.md)
