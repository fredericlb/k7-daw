// chainCreate.ts — creating nodes and lanes in a chain project. A new node is one item in a chain of
// its own with no start and no ports: hidden until it is placed on the graph or wired. A new lane is
// its `tracks/` file plus a `start: true` chain holding it. Nothing here wires anything, but an
// input put in a lane's chain ahead of the lane, which IS its wiring.
// What would be got wrong: the name a caller asked for names the CHAIN; the node is its fresh id.
// See docs/chainFiles.md.

import { stub } from "./editStub";

/** Whether `chains/<name>` is taken by a chain file. */
export const chainTaken = (dir: string, name: string): boolean =>
  stub("song/chainCreate.chainTaken", dir, name);

/** Write a node, given as a node file's k7x text, as the one item of a new loose chain `chain`; its id and file. */
export function createChainNode(
  dir: string,
  chain: string,
  body: string,
): { id: string; rel: string } {
  return stub("song/chainCreate.createChainNode", dir, chain, body);
}

/** A new lane: `tracks/<lane>.k7x` holding `body` (k7x text), and the `start: true` chain that plays it. */
export function createChainLane(
  dir: string,
  lane: string,
  body: string,
): string[] {
  return stub("song/chainCreate.createChainLane", dir, lane, body);
}

/** Put an input, given as a node file's k7x text, in the lane's chain just ahead of the lane — docs/chainFiles.md. */
export function insertChainInput(
  dir: string,
  lane: string,
  body: string,
): { id: string; rel: string } {
  return stub("song/chainCreate.insertChainInput", dir, lane, body);
}
