// shownName.ts — what a person reads for a chain node: `<module> · <chain> · step n` when it is unnamed, its own name otherwise.
// The names are Rust's (the chains crate's shown_name), one call per graph; here a lookup adds the `#port` a name may carry.
// The name itself (a uuid in a chain project) stays what commands, the wire and the files carry. apps/k7-daw-wasm/song/docs/chainFiles.md.

import {
  splitPort,
  type WebChain,
  type WebChainStep,
} from "@studio/graph-model";
import { type ShownStep, shownNameMap } from "@studio/k7-wasm/nodes";

type Step = Partial<Pick<WebChainStep, "module" | "uses" | "place">>;

/** A lookup from any node name (a `#port` kept as written) to what is shown for it; a name no step places is its own. */
export function shownNamesOf(steps: Iterable<Step>): (name: string) => string {
  const slim: ShownStep[] = [...steps].map((s) => ({
    module: s.module,
    uses: s.uses,
    place: s.place,
  }));
  const by = shownNameMap(slim);
  return (name) => {
    const { node, port } = splitPort(name);
    const shown = by[node];
    if (!shown) return name;
    return port ? `${shown} #${port}` : shown;
  };
}

/** …over a graph's chains, which also names a chain HEADED by an unnamed node, keyed by that node's name. */
export const shownNames = (
  chains: Record<string, Pick<WebChain, "use">> | undefined,
): ((name: string) => string) =>
  shownNamesOf(Object.values(chains ?? {}).flatMap((c) => c.use));
