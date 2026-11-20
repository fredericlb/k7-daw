// alternativeSwitch.ts — switching a module to another of its attribute SETS, and saying whether
// the switch reaches a running engine as plain VALUES. When the two sets differ in numeric params
// alone, the answer carries what to set, so the caller can patch the held project instead of
// recompiling it. See packages/song/docs/alternatives.md.

import { stub } from "@studio/song/editStub";

/** The values a switch changed: a PATCH over the plugin's params, or the WHOLE of them when inheritance had to be resolved. */
export interface SwitchedParams {
  params: Record<string, number>;
  whole: boolean;
}

/** Switch a module's set by name; `values` is present only when the switch changed nothing but numbers. */
export function selectModuleAlternative(
  dir: string,
  module: string,
  name: string,
): { path: string; values?: SwitchedParams } {
  return stub(
    "song-edit/alternativeSwitch.selectModuleAlternative",
    dir,
    module,
    name,
  );
}
