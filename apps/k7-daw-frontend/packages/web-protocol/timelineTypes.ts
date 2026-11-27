// timelineTypes.ts — the compiled song as the frontend draws it: the timeline (beats) and the
// picture (seconds), two documents read off one folder, and the compile's diagnostics.
// Re-exported by types.ts, which is what every consumer imports.

import type { EventDoc } from "@studio/k7-wasm/types";

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
// A video clip: every time here is SECONDS, picture being cut against the wall clock — apps/k7-daw-frontend/docs/picture.md.
export interface WebVideoClip {
  id: string;
  name: string;
  src: string; // workspace-relative media path
  /** A browser-decodable copy at full resolution, read instead of `src` by preview and export (song-video/docs/media.md). */
  optimized?: string;
  /** …and a small one for the preview alone. */
  proxy?: string;
  /** An IMAGE, held while the shot is on (song-video/docs/media.md). */
  still?: true;
  start: number; // seconds
  length: number; // seconds (0 == natural)
  sourceFrom: number; // seconds into the source
  fadeIn?: WebVideoFade;
  fadeOut?: WebVideoFade;
  /** How the shot moves in its lane's frame: each property's keys, sorted, in seconds from its start (song-video/docs/animation.md). */
  animate?: WebVideoAnimate;
  /** How fast the source runs under the shot, signed; absent is 1 (song-video/docs/speed.md). */
  speed?: number;
  /** The run this shot alone goes through, by head node — a key of `stretches` (song-video/docs/effects.md). */
  effect?: string;
  /** The timeline that placed this shot here, which is where it is edited (song/docs/timelines.md). */
  timeline?: string;
  /** A subtitle clip's cues and style (mirrors CaptionsDoc, song-video/docs/titles.md). */
  captions?: {
    cues: { start: number; end: number; text: string }[];
    style: {
      font?: string;
      fontSize?: number;
      color?: string;
      background?: string;
      align?: "left" | "center" | "right";
      position?: "bottom" | "top" | "middle";
      margin?: number;
    };
  };
}
/** One keyframe as the compile normalised it; `ease` shapes the segment LEAVING it. */
export interface WebVideoKey {
  t: number;
  v: number;
  ease: string;
}
/** A shot's animated properties — x, y, scale, opacity, rotate — each its keys. */
export type WebVideoAnimate = Partial<
  Record<"x" | "y" | "scale" | "opacity" | "rotate", WebVideoKey[]>
>;
/** A transition ON a placement (mirrors VideoFadeDoc): how long, and which kind — song-video/docs/transitions.md. */
export interface WebVideoFade {
  over: number; // seconds
  with: string; // a modules/video_transitions/ name, "dissolve", or "over"
  /** The STRETCH the shot runs through over the fade — a key of `WebVideoProject.stretches`. */
  chain?: string;
}
/** A transition's two stages (mirrors VideoTransitionDoc), keyed by NAME on the project — its GLSL rides inline. */
export interface WebVideoTransition {
  name: string;
  vertex: string;
  fragment: string;
  params: Record<string, number>;
}
/** A shape module (mirrors ShapeDoc), geometry in px; `id` is the NODE that draws it — apps/k7-daw-wasm/verbs/docs/picture/shapes.md. */
export type WebShape =
  | {
      id: string;
      type: "square";
      x: number;
      y: number;
      w: number;
      h: number;
      color: string;
    }
  | {
      id: string;
      type: "circle";
      x: number;
      y: number;
      r: number;
      color: string;
    }
  | {
      id: string;
      type: "text";
      content: string;
      font: string;
      fontSize: number;
      x: number;
      y: number;
      w?: number;
      color: string;
      align?: "left" | "center" | "right"; // how its lines sit in `w`, or about `x` with none
      background?: string; // a box behind the lines, any CSS colour
      shadow?: boolean; // a soft shadow under the glyphs, for text over a picture
      lineHeight?: number; // a multiple of the font size; 1.2 unsaid
      padding?: number; // px between the lines and their box
    };
/** One pass over the picture (mirrors VideoPluginDoc); its GLSL rides inline and `audioFrom` names a lane — see docs/document.md. */
export interface WebVideoPlugin {
  id: string;
  // No `video_transition`: one is NAMED by a cut and never wired — song-video/docs/transitions.md.
  type: "video_effect" | "shader";
  name: string;
  enabled: boolean;
  params: Record<string, number>;
  shader?: { name: string; vertex: string; fragment: string };
  audioFrom?: string;
  /** The `.cube` LUT the shader reads, workspace-relative (song-video/docs/effects.md). */
  lut?: string;
}
/** A video lane (mirrors VideoTrackDoc): picture, a lens, or the `shapes` wired to it; in draw order, bottom first. */
export interface WebVideoTrack {
  id: string;
  name: string;
  color: string;
  mute: boolean;
  /** Soloed, off `project.k7x`'s `soloed:` — the same list audio reads (apps/k7-daw-wasm/verbs/docs/picture/picture.md). */
  solo: boolean;
  x: number;
  y: number;
  w: number;
  h: number;
  clips: WebVideoClip[];
  shapes: WebShape[];
  // This lane's own run, applied to the layer BEFORE it is composited; absent on a lane no run starts at.
  plugins?: WebVideoPlugin[];
  /** The camera LABEL feeding this lane instead of its placements, "" for the machine's default — apps/k7-daw-wasm/song/docs/camera.md. */
  camera?: string;
  /** …and WHEN that lens opens: `off` never, `auto` while the lane is armed, `on` always. */
  cameraMonitor?: "off" | "auto" | "on";
}

// --- PICTURE: the same folder, read again ----------------------------------------

// Nothing here is a beat, and it is a separate document for that reason — see docs/document.md.

/** A named point on the picture's ruler. `sec` is its position; `tc` the `m:ss.mmm` it is stored under (the delete key). */
export interface WebVideoMarker {
  sec: number;
  name: string;
  tc: string;
}

export interface WebVideoProject {
  name: string;
  /** `trackOrder` as stated, every lane — where the timeline draws each picture row among the sound rows. */
  rowOrder?: string[];
  /** The frame the lanes draw into, in px. */
  viewport: [number, number];
  /** Frames per second. Authoritative for EXPORT; realtime follows the transport's own clock. */
  fps: number;
  /** Where the last picture ends, in seconds — what the timeline sizes itself against. 0 when empty. */
  duration: number;
  markers: WebVideoMarker[];
  /** Already in `layers:` draw order, bottom first — no consumer re-sorts. */
  videoTracks: WebVideoTrack[];
  /** The `RENDER -> VIDEO_OUT` run over the composited frame — the picture's `masterPlugins`, empty by default. */
  framePlugins: WebVideoPlugin[];
  /** What RENDER's `audio` input carries, the export's sound: the mix, or null for a silent video; absent from an older backend, which exports the mix. */
  sound?: "mix" | null;
  /** Every transition a placement NAMES, by name; the built-in dissolve is absent — the renderer owns it. */
  transitions: Record<string, WebVideoTransition>;
  /** Every stretch a fade NAMES, by its head node — the passes a shot runs through on its way in or out. */
  stretches: Record<string, WebVideoPlugin[]>;
  /** Each timeline's own picture lanes at its own positions, `<timeline>><lane>` — drawn while that timeline is open. */
  timelines?: Record<string, WebVideoTrack[]>;
}

// --- Project errors: whether the project even compiles --------------------------

// Everything the compile reports, `file` project-relative and line/col 1-based; a superset of what breaks the Timeline.
export interface Diagnostic {
  file: string;
  line: number;
  col: number;
  message: string;
  severity?: "error" | "warning"; // "error" (default) draws red; "warning" amber
}
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
