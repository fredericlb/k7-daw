// camera.ts — opening a LIVE lens for a layer that names one. Which lens a label names is Rust's rule (`eyes::cameras`), asked through
// the module: a project says the LABEL a person reads, because a deviceId is salted per browser. See apps/k7-daw-wasm/song/docs/camera.md.

import { matchCamera } from "@studio/k7-wasm/eyes";
import type { CameraMatch } from "@studio/k7-wasm/types";

export type { CameraDevice, CameraMatch } from "@studio/k7-wasm/types";
export { matchCamera };

/** The constraint a match becomes. `exact` deliberately NOT used: an id that has rotated since the
 *  page was last open would then fail outright rather than fall back to a picture. */
export const cameraConstraint = (m: CameraMatch): MediaStreamConstraints => ({
  video: m.deviceId ? { deviceId: m.deviceId } : true,
  audio: false,
});

/** An opened lens, or the sentence saying why there is none — never a throw, see `openLens`. */
export interface Lens {
  stream: MediaStream | null;
  problem: string | null;
  /** Let go of what carries the picture beyond its tracks — a remote lens's peer connection and socket. */
  close?: () => void;
  /** How late its picture is, in ms, as last measured — a remote lens only; absent or null is "about none". */
  latencyMs?: () => number | null;
}

/** Let a lens go: its tracks stopped, then whatever carries them closed — a lens left running keeps the camera lit. */
export function releaseLens(lens: Pick<Lens, "stream" | "close">): void {
  for (const track of lens.stream?.getTracks() ?? []) track.stop();
  lens.close?.();
}

/** How a live lane gets its picture. Injected, because the browser that DRAWS a surface is not
 *  always the one that may open a camera: over the LAN a tablet has no secure context, so its
 *  lens arrives over a peer connection instead. See apps/k7-daw-wasm/song/docs/camera.md. */
export type LensOpener = (label: string) => Promise<Lens>;

/** This browser's own camera — the ordinary opener, and the only one on the machine itself. */
export const localLens: LensOpener = async (label) => {
  const media = globalThis.navigator?.mediaDevices;
  if (!media?.getUserMedia)
    return { stream: null, problem: "this browser offers no camera" };
  try {
    // Enumerated FIRST, so a named lens is matched before anything is opened (docs/camera.md).
    const devices = await media.enumerateDevices();
    const match = matchCamera(label, devices);
    const stream = await media.getUserMedia(cameraConstraint(match));
    return { stream, problem: match.problem ?? null };
  } catch (e) {
    return {
      stream: null,
      problem: `camera unavailable: ${(e as Error).message}`,
    };
  }
};
