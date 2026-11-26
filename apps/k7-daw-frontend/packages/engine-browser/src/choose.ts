// choose.ts — which engine a service should drive, given what the account remembers: the page's half of a Rust policy.
// A reader would get wrong that only the address facts are worked out here (a URL parse); the order and the rule are song-schema's.
// See docs/access.md, "Who is taken unasked".

import {
  bestEngineAt,
  type Candidate,
  chooseEngineAt,
} from "@studio/k7-wasm/engine";
import {
  onThisMachine,
  type ProbeTarget,
  type RankedEngine,
  reachedDirectly,
} from "./probe.ts";

export interface Choice<E extends ProbeTarget> {
  /** The engine to use, or null when the user has to be asked (or there is none). */
  engine: RankedEngine<E> | null;
  /** True when several were reachable and none was remembered — a single reachable engine is not a choice, it is the answer. */
  ambiguous: boolean;
}

/** What `bestEngine` reads beyond the probe: both optional, so an engine that reports neither still ranks on its latency. */
export interface RatedTarget extends ProbeTarget {
  assigned?: boolean;
  performance?: { score: number } | null;
}

/** What else `bestEngine` may weigh. */
export interface BestEngineOptions {
  /** The engine's compatibility with the project (0–100, see compat.ts); null when it could not be told, which ranks below any score. */
  compat?: (engineId: string) => number | null;
  /** The engine this account last CHOSE — a person's pick, which a faster machine elsewhere does not overrule. */
  remembered?: string;
}

function candidateOf<E extends RatedTarget>(
  r: RankedEngine<E>,
  compat: BestEngineOptions["compat"],
): Candidate {
  const c: Candidate = {
    id: r.engine.id,
    reachable: !!(r.wsUrl && r.token),
    direct: reachedDirectly(r),
    here: onThisMachine(r),
    assigned: !!r.engine.assigned,
  };
  if (r.latency)
    c.latency = { quality: r.latency.quality, totalMs: r.latency.totalMs };
  const score = r.engine.performance?.score;
  if (score !== undefined) c.performance = score;
  const fit = compat?.(r.engine.id);
  if (fit !== undefined && fit !== null) c.compat = fit;
  return c;
}

/** Pick from what the registry offered: UNREACHABLE engines are excluded, and a remembered one being away does NOT clear the preference. */
export function chooseEngine<E extends RatedTarget>(
  ranked: RankedEngine<E>[],
  rememberedId: string | undefined,
): Choice<E> {
  const { index, ambiguous } = chooseEngineAt(
    ranked.map((r) => candidateOf(r, undefined)),
    rememberedId,
  );
  return { engine: index === null ? null : ranked[index], ambiguous };
}

/** The engine to take when nobody has to be asked, or null when there is none; compatibility first, then the pick, this machine, the link and the machine's rating. */
export function bestEngine<E extends RatedTarget>(
  ranked: RankedEngine<E>[],
  opts: BestEngineOptions = {},
): RankedEngine<E> | null {
  const at = bestEngineAt(
    ranked.map((r) => candidateOf(r, opts.compat)),
    opts.remembered,
  );
  return at === null ? null : ranked[at];
}
