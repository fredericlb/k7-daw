// tapChain.ts — a tap's SUBCHAIN, edited from the tap: add a module (the first one creating the subchain and routing
// the tap to it), remove one, move one. The last one removed takes the subchain with it and puts the tap back on
// the main output, since a tap-fed subchain with no node feeds nothing. Written straight into the node's chain file,
// one write per edit, never through a graph rewrite. See apps/k7-daw-frontend/packages/song/docs/racks.md.

import { stub } from "@studio/song/editStub";

/** Add a node of module `ref` at the end of the tap's subchain — made, and the tap routed to it, when there is none. */
export function addTapChainNode(
  dir: string,
  node: string,
  tap: string,
  ref: string,
): { subchain: string; id: string } {
  return stub("song-edit/tapChain.addTapChainNode", dir, node, tap, ref);
}

/** Remove one node of the tap's subchain; the last one takes the subchain with it and the tap back to the main output. */
export function removeTapChainNode(
  dir: string,
  node: string,
  tap: string,
  id: string,
): { emptied: boolean } {
  return stub("song-edit/tapChain.removeTapChainNode", dir, node, tap, id);
}

/** Move one node of the tap's subchain `by` places (-1 earlier, +1 later), kept inside its nodes. */
export function moveTapChainNode(
  dir: string,
  node: string,
  tap: string,
  id: string,
  by: number,
): void {
  stub("song-edit/tapChain.moveTapChainNode", dir, node, tap, id, by);
}
