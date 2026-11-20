// liveLane.ts — a LIVE lane's state and the lens feeding it: one <video> carrying a camera's
// stream, which chases no clock and holds no clip. Apart from realtime.ts, which is about the
// elements that do. See apps/k7-daw-frontend/packages/song/docs/camera.md.

import { type LensOpener, releaseLens } from "./camera";

/** A live lane's state: the element, the stream once granted, and why there is none otherwise. */
export interface LiveLane {
  el: HTMLVideoElement;
  stream: MediaStream | null;
  problem: string | null;
  /** The lens's own release, beyond stopping its tracks. */
  close?: () => void;
  /** Set once the lane is closed, so a lens still opening releases itself. */
  dropped?: boolean;
}

/** Open the lens a layer names and hand it to `live.el`. Fire and forget: the first frames arrive
 *  whenever the browser and the person granting permission agree, and the lane draws nothing until
 *  they do. Every failure is CARRIED rather than thrown — a refused camera must not take the whole
 *  viewport down with it, and the other layers have pictures to show. See docs/camera.md. */
export async function openCamera(
  label: string,
  live: LiveLane,
  open: LensOpener,
  gone: () => boolean,
) {
  const lens = await open(label);
  // The driver was destroyed while the lens opened (a permission prompt, a slow device): released here, or the lamp stays lit.
  if (gone()) {
    releaseLens(lens);
    return;
  }
  live.close = lens.close;
  live.problem = lens.problem;
  if (!lens.stream) return;
  live.stream = lens.stream;
  live.el.srcObject = lens.stream;
  await live.el.play().catch(() => undefined);
}
