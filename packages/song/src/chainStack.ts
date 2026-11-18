// chainStack.ts — STACK a loose node under another in a chain project: the two become a subchain run
// in place of the one above (`{ sub }`), or the new one joins the subchain that one already sits in.
// The one thing a reader would get wrong: the new node must be LOOSE — alone in its chain, wired to
// nothing — which is what `node new` leaves; its chain file goes, its item moves. docs/chainFiles.md.

import { stub } from "./editStub";

/** Put loose node `node` right under `above`, in one subchain with it; the project-relative files written. */
export function stackChainNode(
  dir: string,
  above: string,
  node: string,
): string[] {
  return stub("song/chainStack.stackChainNode", dir, above, node);
}
