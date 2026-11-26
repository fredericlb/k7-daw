// engine.ts — the page's engine rules from Rust: a text's fingerprint, how an engine fits a project, its link's band, which engine to take, what a warning means.
// A reader would get wrong that none of these runs per frame: each is one JSON call per write, probe, pick or warning.

import { call } from "./abi.ts";
import type {
  BestAnswer,
  Candidate,
  CataloguePlugin,
  Choice,
  Compatibility,
  EngineLatency,
  PluginNeed,
  WarningAnswer,
} from "./types/index.ts";

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

/** A short fingerprint of a text, the same on every side of a write: what a write sends as the base it was edited from. */
export const textHash = (text: string): string =>
  call<string>({ op: "text.hash", args: { text } });

/** The share of a project's plugins an engine's catalogue has, and which are missing. */
export const compatibility = (
  needs: PluginNeed[],
  catalogue: CataloguePlugin[],
): Compatibility =>
  call<Compatibility>({ op: "engine.compat", args: { needs, catalogue } });

/** A link's latency in ms and its band: the round trip plus twice the buffer period. */
export const engineLatency = (
  networkMs: number,
  bufferFrames: number,
  sampleRate: number,
): EngineLatency =>
  call<EngineLatency>({
    op: "engine.latency",
    args: { networkMs, bufferFrames, sampleRate },
  });

/** What an engine warning's prose is about, and the missing thing it reports, if any. */
export const readWarning = (message: string): WarningAnswer =>
  call<WarningAnswer>({ op: "engine.warning", args: { message } });

/** The engine to use from what the registry offered, as an index; null index when the user has to be asked. */
export const chooseEngineAt = (
  candidates: Candidate[],
  remembered: string | undefined,
): Choice =>
  call<Choice>({ op: "engine.choose", args: { candidates, remembered } });

/** The engine to take when nobody is asked, as an index into the candidates, or null when none may be taken. */
export const bestEngineAt = (
  candidates: Candidate[],
  remembered: string | undefined,
): number | null =>
  call<BestAnswer>({ op: "engine.best", args: { candidates, remembered } })
    .index;
