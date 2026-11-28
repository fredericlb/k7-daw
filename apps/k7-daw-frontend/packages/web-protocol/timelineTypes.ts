// timelineTypes.ts — the compiled song as the frontend draws it: the timeline (beats) and the
// picture (seconds), two documents read off one folder, and the compile's diagnostics.
// Re-exported by types.ts, which is what every consumer imports.

import type { Diagnostic, EventDoc, WebVideoClip } from "@studio/k7-wasm/types";

// --- Timeline: the compiled, flat project (mirrors the brain's ProjectDoc) ------

// GENERATED from the Rust views (apps/k7-daw-wasm/song/src/views/web, ts-rs): the page's WebProject and the tracks, clips and notes in it.
export type {
  WebClip,
  WebGroup,
  WebMarker,
  WebMixer,
  WebNote,
  WebPlugin,
  WebProject,
  WebTimeline,
  WebTrack,
} from "@studio/k7-wasm/types";

/** A placement's own fields, as authored — the engine document's `EventDoc`. */
export type WebEvent = EventDoc;

// Compiled by the backend, so the Timeline knows nothing about chains or modules — see docs/document.md.

// A downsampled waveform for one audio clip plus the bar/beat grid aligning it — for the audio-clip editor.
export interface WebWaveformGrid {
  t: number; // seconds from the region start
  bar: boolean; // a bar boundary (vs. a plain beat)
  label?: string; // bar number, on bar boundaries
}
// A warp marker in seconds from the region start: `orig` where the point is drawn, `target` where its beat lands.
export interface WebWarpMark {
  orig: number;
  target: number;
  /** The FILE second this point pins, which a surface cannot invert from two numbers — see docs/document.md. */
  src: number;
}
export interface WebWaveform {
  rms: number[]; // 0..1 linear per bucket (feeds the recorder Waveform's dB envelope)
  peak: number[]; // 0..1 per bucket
  /** The three band peaks in `peak`'s buckets, for the spectrum look; absent on a sidecar analysed without them. */
  low?: number[];
  mid?: number[];
  high?: number[];
  seconds: number; // duration of the shown region
  intervalMs: number; // seconds / rms.length * 1000 (envelope sample spacing)
  startBeat: number; // the clip's start in the arrangement
  clipBeats: number; // beats the region spans
  grid: WebWaveformGrid[];
  warpMarks: WebWarpMark[]; // stretch markers, in region seconds (empty == not warped)
  /** `display = (file − inPoint) × stretch` — the mapping a GESTURE has to go back through; see docs/document.md. */
  sourceMap: { inPoint: number; stretch: number; zeroBeat: number };
  /** The span of the source this clip IS, sent only when the picture is WIDER than it — see docs/document.md. */
  sourceWindow?: { from: number; to: number };
  // Display seconds where the onsets ARE, corrections applied — the one list every surface agrees on.
  transients: number[];
  // The same list before corrections, and the corrections themselves, in FILE seconds — see docs/document.md.
  detected?: number[];
  onsetEdits?: { added?: number[]; removed?: number[] };
}

// Audition one sample through the engine: a media path, a clip id, or a slice window in seconds into a source.
export type PreviewReq =
  | { kind: "source"; source: string }
  | { kind: "clip"; clip: string }
  | { kind: "slice"; source: string; from: number; to: number };

// A plugin's FACTORY preset, shaped by the engine that discovers it.
export type { PresetInfo } from "@studio/engine-protocol";

// Progress of the background waveform prewarm (for the status-line spinner).
export interface WaveformStatus {
  warming: boolean;
  done: number;
  total: number;
}
// The picture's types are GENERATED from the Rust document (apps/k7-daw-wasm/verbs/src/picture/doc.rs, ts-rs): every time in them is SECONDS — apps/k7-daw-frontend/docs/picture.md.
export type {
  SceneFade as WebVideoFade,
  SceneKey as WebVideoKey,
  SceneShape as WebShape,
  SceneTransition as WebVideoTransition,
  WebVideoClip,
  WebVideoMarker,
  WebVideoPlugin,
  WebVideoProject,
  WebVideoTrack,
} from "@studio/k7-wasm/types";

/** A shot's animated properties — x, y, scale, opacity, rotate — each its keys. */
export type WebVideoAnimate = NonNullable<WebVideoClip["animate"]>;

// --- PICTURE: the same folder, read again ----------------------------------------

// Nothing here is a beat, and it is a separate document for that reason — see docs/document.md.

// --- Project errors: whether the project even compiles --------------------------

// Everything the compile reports, `file` project-relative and line/col 1-based; GENERATED from the Rust `Diagnostic`.
export type { Diagnostic } from "@studio/k7-wasm/types";
/** A project root written by a newer k7: the levels the "update k7" message names (apps/k7-daw-wasm/song-schema/docs/formatLevel.md). */
export interface FormatTooNew {
  fileLevel: number;
  buildLevel: number;
}
export interface ProjectErrors {
  errors: string[]; // flat messages — the count badge
  diagnostics: Diagnostic[]; // located, for the editor's inline squiggles
  formatTooNew?: FormatTooNew; // set when the compile refused a newer format, so the UI says "update k7"
}
/** A document route's 400: the compiler's message, plus `code` and the levels when it is a format refusal. */
export interface CompileErrorBody {
  error: string;
  code?: "format-too-new";
  fileLevel?: number;
  buildLevel?: number;
}
