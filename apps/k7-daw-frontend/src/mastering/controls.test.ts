// The wiring of the mastering vocabulary: Rust's table gains its labels here, and its gate and modules answer through the facade.
// The table's rules (parts, ranges, the required ten) are tested in the chains crate: apps/k7-daw-wasm/chains/src/master_controls/tests.rs.

import { describe, expect, it } from "vitest";
import {
  MASTER_CONTROLS,
  mappedModules,
  masterControl,
  unmappedControls,
} from "./controls";

describe("the mastering surface's controls", () => {
  it("gives every control of the table a label and finds one by its id", () => {
    expect(MASTER_CONTROLS).toHaveLength(13);
    for (const c of MASTER_CONTROLS) expect(typeof c.label()).toBe("string");
    expect(masterControl("limiter.ceiling")?.range).toEqual([-12, 0]);
  });

  it("answers the gate and the mapped modules through the facade", () => {
    const required = MASTER_CONTROLS.filter((c) => !c.optional);
    expect(unmappedControls({})).toEqual(required);
    const one = Object.fromEntries(
      MASTER_CONTROLS.map((c) => [c.id, { module: "qm", param: "x" }]),
    );
    expect(unmappedControls(one)).toHaveLength(0);
    expect(mappedModules(one)).toEqual(["qm"]);
    expect(mappedModules(undefined)).toEqual([]);
  });
});
