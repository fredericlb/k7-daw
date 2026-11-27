// gainTrim.ts — the header meter's Fix button: where the OUT node's Volume has to land for a clipped pass to have peaked at -6 dB.
// The arithmetic is Rust's (song/src/reads/misc.rs); see apps/k7-daw-frontend/docs/masterMeter.md.

import { FIX_TARGET_DB } from "@studio/k7-wasm/consts";
import { fixedMaster, trimFor } from "@studio/k7-wasm/reads";

/** Where a fixed mix should peak: -6 dB gives the headroom back, and is where the meter already turns amber (`HOT_DB`). */
export const TARGET_DB = FIX_TARGET_DB;

export { fixedMaster, trimFor };
