// videoTake.ts — recording a picture lane's input into a clip: which container the browser will actually give us.
// The names the take gets (extension, stamp, id, the `picture add` line) are Rust's: @studio/k7-wasm/intake. See docs/picture.md.

/** The containers to try, best first. WebM is the one every browser that records video records. */
export const VIDEO_MIMES = [
  "video/webm;codecs=vp9",
  "video/webm;codecs=vp8",
  "video/webm",
  "video/mp4",
] as const;

/** The first container this browser records, or undefined — which asks MediaRecorder to choose. */
export function pickVideoMime(
  supported: (m: string) => boolean,
): string | undefined {
  return VIDEO_MIMES.find(supported);
}
