// track.ts — what a track's name declares (`[position.][prefix.]label`), answered by the Rust crate (ops/track.rs).
// A reader would get wrong that the prefix table is here: it is `TRACK_PREFIXES` in the generated constants.

import { call } from "./abi.ts";
import type { TrackKind } from "./types/index.ts";

export type { TrackKind };

/** The name with its position and instrument stripped, what a view SHOWS; the whole name when nothing would remain. */
export const trackLabel = (name: string): string =>
  call<string>({ op: "track.label", args: { name } });

/** The instrument a name declares — its code, family and tooltip label — or null when it declares none. */
export const trackKind = (name: string): TrackKind | null =>
  call<TrackKind | null>({ op: "track.kind", args: { name } });
