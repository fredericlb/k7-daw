// rackTaps.ts — editing a rack NODE's taps: the note each sound answers to and the library sample
// it plays. They are written into the node's own file, never the module's: a rack module is the
// sampler, and each node of it is a kit (packages/song/docs/racks.md). On an alternative other than
// the first, into that set — a knob and a tap follow the same rule.

import { stub } from "@studio/song/editStub";
import type { resolvedRack } from "@studio/song/moduleResolve";

/** The node's effective rack, refused when the node is not a rack at all. */
export function kitOf(
  dir: string,
  node: string,
): ReturnType<typeof resolvedRack> {
  return stub("song-edit/rackTaps.kitOf", dir, node);
}

/** Set one tap's note or sample on the node; a `null` sample clears it. The tap is made when it is not there. */
export function setTap(
  dir: string,
  node: string,
  symbol: string,
  change: { note?: number; sample?: string | null; mute?: boolean },
): string {
  return stub("song-edit/rackTaps.setTap", dir, node, symbol, change);
}

/** Where a positioned add lands: next to `symbol`'s pad, on its `side` in note order. */
export interface TapNear {
  symbol: string;
  side: "before" | "after";
}

/** Add a tap: `symbol` or the next free `T<n>`; on note `near` when it is one, beside a tap when it names one, else after the kit's highest. */
export function addTap(
  dir: string,
  node: string,
  symbol?: string,
  near?: TapNear | number,
): { symbol: string; note: number } {
  return stub("song-edit/rackTaps.addTap", dir, node, symbol, near);
}

/** Rename a tap the node itself states, in place — its note and sample unchanged; an inherited one is refused. */
export function renameTap(
  dir: string,
  node: string,
  from: string,
  to: string,
): string {
  return stub("song-edit/rackTaps.renameTap", dir, node, from, to);
}

/** Remove a tap the node itself states; one it inherits from its module is refused by name. */
export function removeTap(dir: string, node: string, symbol: string): string {
  return stub("song-edit/rackTaps.removeTap", dir, node, symbol);
}

/** Replace the kit the node itself states with `taps` — a module's own taps still merge in under it. */
export function setKit(
  dir: string,
  node: string,
  taps: Record<string, { note: number; sample?: string }>,
): string {
  return stub("song-edit/rackTaps.setKit", dir, node, taps);
}

/** A node's taps by name: a rack's sounds, or an SFZ kit's pads; none for any other node. */
export function tapsOf(dir: string, node: string): string[] {
  return stub("song-edit/rackTaps.tapsOf", dir, node);
}

/** Route a tap — a rack's sound or a kit's pad — to a subchain of the node's chain file; `null` returns it to the main output. */
export function setTapOut(
  dir: string,
  node: string,
  tap: string,
  subchain: string | null,
): string {
  return stub("song-edit/rackTaps.setTapOut", dir, node, tap, subchain);
}
