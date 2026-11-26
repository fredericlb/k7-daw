// picture.ts — the frame's shape and the words that set a picture render, answered by the Rust crate (ops/picture.rs).
// A reader would get wrong that the vocabularies are here: they are generated constants, re-exported so a caller imports one door.

import { call } from "./abi.ts";
import type {
  Aspect,
  ExportFormat,
  ExportQuality,
  RenderScene,
  ResolvedVideo,
  VideoSettings,
} from "./types/index.ts";

export {
  ASPECTS,
  EXPORT_FORMATS,
  EXPORT_QUALITIES,
  VIDEO_FORMATS,
} from "./consts.generated.ts";
export type { Aspect, ExportFormat, ExportQuality, RenderScene, VideoSettings };
export type VideoFormat = ExportFormat;

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `picture.${op}`, args });

/** Which shape a viewport IS, by ratio rather than by exact size; null when it matches none. */
export const aspectOf = (w: number, h: number): Aspect | null =>
  ask("aspect_of", { w, h });

/** The settings `words` say, or null when one is not understood or the range runs backwards. */
export const parseVideoSettings = (
  words: readonly string[],
): VideoSettings | null => ask("parse_settings", { words });

/** The range and file size `settings` resolve to on a scene: the whole song by default, a height alone taking the viewport's aspect. */
export const resolveVideoSettings = (
  settings: VideoSettings,
  scene: RenderScene,
): ResolvedVideo => ask("resolve_settings", { settings, scene });
