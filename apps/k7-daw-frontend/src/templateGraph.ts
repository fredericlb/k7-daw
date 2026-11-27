// templateGraph.ts — a library chain FILE drawn in the browser from its own text: one run from `@in` to `@out` (ops/reads.rs).
// A reader would get wrong that this drawing has no gestures: the server's graph verbs reach only a folder chain (docs/chainsBuilder.md).

import { templateGraph as read } from "@studio/k7-wasm/reads";
import type { WebGraph } from "./types";

/** The chain file's text as a one-chain graph, or the first reason it does not read. */
export const templateGraph = (
  name: string,
  text: string,
): { graph: WebGraph } | { error: string } => read<WebGraph>(name, text);
