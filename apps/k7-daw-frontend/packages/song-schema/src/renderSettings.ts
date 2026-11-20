// renderSettings.ts — the words that set a picture render, read in ONE place: the browser's
// `project export video` and the server's `picture render` take the same line, so they must read
// it alike. Here and not in ui-eyes so the command layer reads them without a WebGL package.
// See apps/k7/backend-rs/docs/file-engine-jobs.md.

/** The quality presets an export may ask for, lowest first; `max` is mediabunny's "very high". */
export const EXPORT_QUALITIES = ["low", "medium", "high", "max"] as const;
export type ExportQuality = (typeof EXPORT_QUALITIES)[number];

/** The containers an export leaves in, the default first; whether this build encodes one is ui-eyes' exportSupport.ts. */
export const EXPORT_FORMATS = ["webm", "mp4"] as const;
export type ExportFormat = (typeof EXPORT_FORMATS)[number];

/** The containers the picture leaves in, the default first, each one an export format the probe knows. */
export const VIDEO_FORMATS = [
  "webm",
  "mp4",
] as const satisfies readonly ExportFormat[];
export type VideoFormat = (typeof VIDEO_FORMATS)[number];

/** The render's own settings, each optional: what a line leaves out is the viewport's, `high`, the whole song. */
export interface VideoSettings {
  /** A height in px (`720p`), the width following the viewport's aspect — or both (`1280x720`). */
  size?: { height: number } | { width: number; height: number };
  quality?: ExportQuality;
  from?: number; // seconds
  to?: number; // seconds
}

/** What a server render is asked for: the container, whether the mixdown is married in, and the export's own settings. */
export type PictureRenderOptions = {
  format: VideoFormat;
  audio: boolean;
} & VideoSettings;

/** The range and the file size `settings` resolve to on a scene: the whole song by default, a height alone taking the viewport's aspect. */
export function resolveVideoSettings(
  settings: VideoSettings,
  scene: {
    viewport: readonly [number, number];
    fps: number;
    durationSec: number;
  },
): { from: number; frameCount: number; size?: [number, number] } {
  const from = settings.from ?? 0;
  const to = settings.to ?? scene.durationSec;
  const frameCount = Math.max(1, Math.round((to - from) * scene.fps));
  if (!settings.size) return { from, frameCount };
  const [vw, vh] = scene.viewport;
  const size: [number, number] =
    "width" in settings.size
      ? [settings.size.width, settings.size.height]
      : [Math.round((settings.size.height * vw) / vh), settings.size.height];
  return { from, frameCount, size };
}

/** Seconds, or `m:ss(.s)` — the picture's own spelling of a time; null for anything else. */
function parseClock(s: string): number | null {
  const m = /^(?:(\d+):)?(\d+(?:\.\d+)?)$/.exec(s);
  if (!m) return null;
  const sec = Number(m[2]) + (m[1] ? Number(m[1]) * 60 : 0);
  return Number.isFinite(sec) ? sec : null;
}

// One `key=value` word into `into`; false when it is not one this understands.
function setting(word: string, into: VideoSettings): boolean {
  const [key, value = ""] = word.split("=", 2);
  if (key === "size") {
    const p = /^(\d+)p$/.exec(value);
    const wh = /^(\d+)x(\d+)$/.exec(value);
    if (p) into.size = { height: Number(p[1]) };
    else if (wh) into.size = { width: Number(wh[1]), height: Number(wh[2]) };
    else return false;
    return true;
  }
  if (key === "quality") {
    if (!(EXPORT_QUALITIES as readonly string[]).includes(value)) return false;
    into.quality = value as ExportQuality;
    return true;
  }
  if (key === "from" || key === "to") {
    const t = parseClock(value);
    if (t === null) return false;
    into[key] = t;
    return true;
  }
  return false;
}

/** The settings `words` say, or null when one is not understood or the range runs backwards. */
export function parseVideoSettings(
  words: readonly string[],
): VideoSettings | null {
  const out: VideoSettings = {};
  for (const w of words) if (!setting(w.toLowerCase(), out)) return null;
  if (out.from !== undefined && out.to !== undefined && out.to <= out.from)
    return null;
  return out;
}
