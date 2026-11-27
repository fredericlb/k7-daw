// rackFor.ts — the rack a track's signal reaches, and the key names its sounds give the piano roll's gutter (song/src/reads/graph.rs).
// A reader would get wrong that bypassed steps count: the rack a lane plays is the first in signal order, whether or not it is on.

import type { WebGraph, WebRackSound } from "@studio/graph-model";
import { rackKeyNames as names, rackFor as read } from "@studio/k7-wasm/reads";

/** The sounds of the first rack downstream of `trackId`, or null — walked in signal order, bypassed steps counting, and cycle-safe. */
export const rackFor = (
  graph: WebGraph | null | undefined,
  trackId: string,
): WebRackSound[] | null =>
  graph && trackId ? read<WebRackSound>(graph, trackId) : null;

/** Each rack key's name for the roll's key column: its sample's file name, else its clip, else its symbol — several on one key joined. */
export const rackKeyNames = (
  rack: readonly WebRackSound[],
): Map<number, string> => names(rack);
