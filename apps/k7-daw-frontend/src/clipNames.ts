// clipNames.ts — what a clip is CALLED on screen. A take's id (`gtr_9.1_261030T172747`) and a new
// sequence's (`keys-seq-1`) are generated, so they read as their lane plus a take bar or a number;
// a clip someone named is shown as named. The id stays the tooltip's — docs/timeline.md. The split is Rust's (song/src/reads/misc.rs).

import { type ClipNameParts, clipNameParts } from "@studio/k7-wasm/reads";
import { trackLabel } from "@studio/k7-wasm/track";

export type { ClipNameParts };
export { clipNameParts };

/** The words a shown name is spelled with — the app's messages, passed in so this stays pure. */
export interface ClipNameWords {
  take: (p: { lane: string; bar: number }) => string;
  sequence: (p: { lane: string; n: number }) => string;
}

/** A lane token as its lane is labelled, from the lanes' ids — a take's token is its lane id, made filename-safe. */
export function laneNamer(
  tracks: readonly { id: string; name: string }[],
): (token: string) => string {
  const by = new Map<string, string>();
  for (const t of tracks) {
    const label = trackLabel(t.name || t.id);
    by.set(t.id, label);
    by.set(t.id.replace(/[^A-Za-z0-9._-]/g, "-"), label);
  }
  return (token) => by.get(token) ?? token;
}

/** The name a clip is shown by: a generated id read as its lane and bar or number, an authored one as written. */
export function clipShownName(
  ref: string,
  laneName: (token: string) => string,
  words: ClipNameWords,
  // The label of the lane the clip sits on, for an id whose lane token names no lane any more (a lane renamed since the take).
  ownLane?: string,
): string {
  const p = clipNameParts(ref);
  if (p.kind === "named") return p.name;
  const known = laneName(p.lane);
  const lane = known === p.lane && ownLane ? ownLane : known;
  if (p.kind === "sequence") return words.sequence({ lane, n: p.n });
  const base = words.take({ lane, bar: p.bar });
  return p.copy ? `${base} (${p.copy})` : base;
}
