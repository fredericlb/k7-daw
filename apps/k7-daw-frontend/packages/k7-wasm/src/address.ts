// address.ts — a project's address read and written by the Rust crate (song-schema's address.rs); the names it reserves are generated constants.
// A reader would get wrong that no origin is here: a path comes back, and the page joins its own origin (web-protocol's projects.ts).

import { call } from "./abi.ts";
import type { BenchRef } from "./types/index.ts";

export {
  ABOUT_PAGE,
  BENCH_PROJECT,
  BENCH_REPO,
  BENCH_WORKSPACE,
  DROPPABLE_PROJECTS,
  EXAMPLES_WORKSPACE,
  EXTERNAL_SEGMENT,
  LAN_REPO,
  LIBRARIES_PAGE,
  LOCAL_REPO,
  MEDIA_PAGE,
  PROJECT_SUFFIX,
  SETTINGS_PAGE,
  SURFACES_PAGE,
  TASKS_PAGE,
} from "./consts.generated.ts";
export * from "./ops/address.ts";
export type { LanSurface, Launch, LocalTarget } from "./types/index.ts";

/** The library chain a bench project names: null for the bare rig, undefined for no bench. */
export function benchChainOf(project: string): string | null | undefined {
  const bench = call<BenchRef | null>({
    op: "address.bench_chain",
    args: { name: project },
  });
  return bench ? bench.chain : undefined;
}
