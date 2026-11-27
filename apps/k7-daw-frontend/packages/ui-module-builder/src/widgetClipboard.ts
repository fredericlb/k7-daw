// widgetClipboard.ts — the one clip a copy holds, at module level so a copy survives switching page or module.
// Cut, copy and paste themselves are Rust's (`@studio/k7-wasm/panel`); positions in a clip are relative to its top-left.

import type { Clip } from "@studio/k7-wasm/panel";

let held: Clip | null = null;

/** The clip a paste would drop, or null when nothing was copied this session. */
export const heldClip = (): Clip | null => held;

export const holdClip = (clip: Clip): void => {
  held = clip;
};
