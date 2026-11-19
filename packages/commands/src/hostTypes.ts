// hostTypes.ts — the shapes a CommandHost member trades, and the one error a member may throw.
// Types are re-exported, never redefined; the class is by name, so a server's own error is caught the same.

import type {
  VideoFormat,
  VideoSettings,
} from "@studio/song-schema/renderSettings";
import type { VideoScene } from "@studio/ui-eyes/scene";

export type { ColdStep, RunColdStep } from "@studio/audio-passes/coldPass";
export type {
  AudioDeviceInfo,
  PluginInfo,
} from "@studio/engine-client/engineClient";
export type {
  Renderer,
  RenderManifest,
  RenderOptions,
} from "@studio/take-render/renderTypes";
export type { VideoFacts } from "@studio/video-prep/plan";

/** What the sound extract found: the frames written, at which rate, over how many channels. */
export interface SoundFacts {
  frames: number;
  sampleRate: number;
  channels: number;
}

/** One song of `playlist stems` as it ended: the first four are said by their state, a failure by its first line. */
export interface PlaylistStemsSong {
  song: string;
  state: "done" | "held" | "absent" | "failed";
  files?: string[];
  error?: string;
}

/** What `prepare` is asked for: one clip's copies, named as the clip file says them (`source` is its own text). */
export interface PrepareAsk {
  at: string;
  clip: string;
  source: string;
  optimized: "remux" | "transcode" | null;
  proxy: boolean;
}

/** What `prepare` answers: `started` is false when that source is being prepared; `line` is a host that ran it inline saying so. */
export interface PrepareAnswer {
  started: boolean;
  line?: string;
}

/** What a picture render is asked for: the scene as the browser's export builds it, the settings, and whether the mixdown is married in. */
export interface PictureAsk {
  scene: VideoScene;
  format: VideoFormat;
  settings: VideoSettings;
  audio: boolean;
}

/** The tool is not installed; `why` says what it was for, and the message is shown to users. */
export class NoFfmpegError extends Error {
  readonly bin: string;
  constructor(
    bin: string,
    why = "this needs ffmpeg. Install it, or point FFMPEG_BIN / FFPROBE_BIN at it.",
  ) {
    super(`${bin} is not installed — ${why}`);
    this.name = "NoFfmpegError";
    this.bin = bin;
  }
}

/** True for a NoFfmpegError of any package or realm: the server's class and this one share only a name. */
export function isNoFfmpeg(e: unknown): boolean {
  return (
    typeof e === "object" &&
    e !== null &&
    (e as { name?: unknown }).name === "NoFfmpegError"
  );
}
