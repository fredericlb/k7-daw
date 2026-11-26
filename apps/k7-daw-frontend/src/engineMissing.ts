// engineMissing.ts — which engine warnings mean the project is playing WITHOUT something it names:
// a file the engine refused or could not open, a plugin it could not create. Rust reads them off the
// warning's text (song-schema's `engine` module), since the engine sends no code for these.
// The one thing a reader would get wrong: these are not fatal — the project plays, a sound short.

import { type MissingItem, readWarning } from "@studio/k7-wasm/engine";

export type { MissingItem };

/** The missing thing an engine warning reports, or null when the warning is about something else. */
export const missingOf = (message: string): MissingItem | null =>
  readWarning(message).missing;

/** A stable key for one item, so the same file named by two tracks is listed once. */
export const missingKey = (i: MissingItem): string =>
  `${i.why}\u0000${i.what}\u0000${i.name}`;
