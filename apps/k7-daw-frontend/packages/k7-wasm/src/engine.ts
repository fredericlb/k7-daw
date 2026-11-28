// engine.ts — the page's engine rules from Rust: a text's fingerprint, how an engine fits a project, its link's band, which engine to take, what a warning means.
// A reader would get wrong that none of these runs per frame: each is one JSON call per write, probe, pick or warning.

import { call } from "./abi.ts";
import type { BestAnswer, Candidate } from "./types/index.ts";

export * from "./ops/engine.ts";
export type {
  Candidate,
  CataloguePlugin,
  Choice,
  Compatibility,
  Duplicated,
  EngineLatency,
  MissingItem,
  PluginNeed,
  Quality,
  WarningAnswer,
  WarningKind,
  Why,
} from "./types/index.ts";

/** The engine to take when nobody is asked, as an index into the candidates, or null when none may be taken. */
export const bestEngineAt = (
  candidates: Candidate[],
  remembered: string | undefined,
): number | null =>
  call<BestAnswer>({ op: "engine.best", args: { candidates, remembered } })
    .index;
