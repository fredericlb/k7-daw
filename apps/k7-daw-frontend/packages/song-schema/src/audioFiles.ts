// audioFiles.ts — which extensions are video, and the extension of a name. `isAudio` is the Rust crate's (`@studio/k7-wasm/schema`).
// A reader would get wrong that `.webm` is on both lists: a name alone cannot say what a container holds.

export const VIDEO_EXT = new Set([".mp4", ".mov", ".mkv", ".webm", ".avi"]);

/** The extension, lowercased and with its dot, or "" — a leading dot is a whole name, not one. */
export function extOf(name: string): string {
  const i = name.lastIndexOf(".");
  return i > 0 ? name.slice(i).toLowerCase() : "";
}

export const isVideo = (name: string): boolean => VIDEO_EXT.has(extOf(name));
