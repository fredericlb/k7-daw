// audioFiles.ts — which extensions are audio and which are video. One list, because a file dropped on
// the timeline, the verbs' media members and a set list's records judge a file by it.

export const AUDIO_EXT = new Set([
  ".wav",
  ".flac",
  ".aiff",
  ".aif",
  ".mp3",
  ".ogg",
  ".opus",
  ".m4a",
  ".webm",
]);

export const VIDEO_EXT = new Set([".mp4", ".mov", ".mkv", ".webm", ".avi"]);

/** The extension, lowercased and with its dot, or "" — a leading dot is a whole name, not one. */
export function extOf(name: string): string {
  const i = name.lastIndexOf(".");
  return i > 0 ? name.slice(i).toLowerCase() : "";
}

export const isAudio = (name: string): boolean => AUDIO_EXT.has(extOf(name));
export const isVideo = (name: string): boolean => VIDEO_EXT.has(extOf(name));
