// graphEdit.ts — the graph verbs: each a `graph.k7x` writer from graphFile.ts, run on
// the file, or — in a chain project — on its chain files read as segments and written back. The
// writers are the same in both, which is why a chain project needs no verbs of its own.
// What would be got wrong: in a chain project a non-lane node is its id, and renaming one is a no-op.
// See apps/k7-daw-frontend/packages/song/docs/chainFiles.md and docs/projectEdit.md.

import { stub } from "@studio/song/editStub";
import type { GraphFile } from "@studio/song-schema/yaml/graphSchema";

/** The graph a writer reads, parsed, or thrown as unreadable — the file, or a chain project's chains. */
export function readGraphValue(dir: string): GraphFile {
  return stub("song-edit/graphEdit.readGraphValue", dir);
}

/** Bypass (or re-enable) a module — off-then-on returns the file to what it was, and no chain need be named. */
export function setModuleBypass(
  dir: string,
  module: string,
  bypassed: boolean,
): string {
  return stub("song-edit/graphEdit.setModuleBypass", dir, module, bypassed);
}

/** Mute (or unmute) a LANE through its bypass, giving the graph an entry for a lane it does not name yet — a picture lane often has none. */
export function setLaneBypass(
  dir: string,
  lane: string,
  bypassed: boolean,
): string {
  return stub("song-edit/graphEdit.setLaneBypass", dir, lane, bypassed);
}

/** Set (or clear) one `bypassed:` entry the graph need not hold as a node — a picture lane's `#audio` half (song docs/videoSound.md). */
export function setBypassEntry(
  dir: string,
  name: string,
  bypassed: boolean,
): string {
  return stub("song-edit/graphEdit.setBypassEntry", dir, name, bypassed);
}

/** Take a node out of the SIGNAL PATH, joining what fed it to what it fed, and leave it in the graph wired to nothing. */
export function unlinkChainNode(
  dir: string,
  name: string,
): { rewired: string[] } {
  return stub("song-edit/graphEdit.unlinkChainNode", dir, name);
}

/** The lane whose chain plays `node`, or null — the FIRST when several reach it, docs/projectEdit.md. */
export function laneOfNode(dir: string, node: string): string | null {
  return stub("song-edit/graphEdit.laneOfNode", dir, node);
}

/** Rename a node: its file, its preset, and every trace of it in the chains — the order is load-bearing, docs/projectEdit.md. */
export function renameChainNode(
  dir: string,
  from: string,
  to: string,
): { moved: string[]; rewired: string[] } {
  return stub("song-edit/graphEdit.renameChainNode", dir, from, to);
}

/** Delete a node: its file, its preset, and every trace of it in the chains — files first, docs/projectEdit.md. */
export function deleteChainNode(
  dir: string,
  name: string,
): { rewired: string[]; removed: string[] } {
  return stub("song-edit/graphEdit.deleteChainNode", dir, name);
}

/** Put a module in the graph wired to nothing — dropped in, to be connected after. */
export function placeChainNode(dir: string, module: string): string {
  return stub("song-edit/graphEdit.placeChainNode", dir, module);
}

/** Pin every node in `at` that has no pin yet — one write, for a graph recording where it drew itself. */
export function setChainNodePositions(
  dir: string,
  at: Record<string, { x: number; y: number }>,
): string {
  return stub("song-edit/graphEdit.setChainNodePositions", dir, at);
}

/** Hand EVERY node back to the layout — `graph organize` clearing the pins in one write. */
export function clearChainNodePositions(dir: string): string {
  return stub("song-edit/graphEdit.clearChainNodePositions", dir);
}

/** Pin a node's place on the graph — or, with null, hand it back to the layout. */
export function setChainNodePosition(
  dir: string,
  name: string,
  at: { x: number; y: number } | null,
): string {
  return stub("song-edit/graphEdit.setChainNodePosition", dir, name, at);
}

/** Draw a new arrow from `from` to `to` (each optionally `name#port`) — the patch-cable gesture's write. */
export function connectChainNodes(
  dir: string,
  from: string,
  to: string,
): string {
  return stub("song-edit/graphEdit.connectChainNodes", dir, from, to);
}

/** Take the arrow from `from` to `to` out — the patch cable pulled and dropped on the canvas. */
export function disconnectChainNodes(
  dir: string,
  from: string,
  to: string,
): string {
  return stub("song-edit/graphEdit.disconnectChainNodes", dir, from, to);
}

/** Move ONE end of an existing arrow — one read and one write, so the cable can never be left half-gone. */
export function replugChainNodes(
  dir: string,
  from: string,
  to: string,
  newFrom: string,
  newTo: string,
): string {
  return stub(
    "song-edit/graphEdit.replugChainNodes",
    dir,
    from,
    to,
    newFrom,
    newTo,
  );
}

/** Insert a module into the arrow from `before` to `after` — the two ends the graph was dropped on. */
export function insertChainStep(
  dir: string,
  before: string,
  after: string | undefined,
  module: string,
): string {
  return stub(
    "song-edit/graphEdit.insertChainStep",
    dir,
    before,
    after,
    module,
  );
}
