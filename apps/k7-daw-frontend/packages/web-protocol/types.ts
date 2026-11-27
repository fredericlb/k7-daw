// types.ts — the API contract between k7's backend and frontend, one module per concern and all of
// them re-exported here, which is the path every consumer imports.

export { MENU_ENTITIES } from "@studio/k7-wasm/consts";
// The palette's catalogue is generated from the verbs crate (`verbs::palette`); the types and the entity list come from its module.
export type {
  ArgClass,
  ArgProvider,
  CommandArg,
  CommandInfo,
  MenuEntity,
} from "@studio/k7-wasm/types";
export * from "./engineAccess.ts";
export * from "./machineTypes.ts";
export * from "./timelineTypes.ts";
export * from "./workspaceTypes.ts";

// How good a link is — Q1/Q2/Q3, off a measured latency — lives in ./quality; see docs/quality.md.
// How capable the machine behind it is — one integer, 1000 the reference — lives in ./performance.
