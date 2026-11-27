// rackBinds.test.ts — how a rack pad's `bind` is spelled and read back, through the package's one import site.
// The pads a rack node draws are the `chains` crate's (src/tests/lanes.rs, src/node_ui/tests.rs).

import { describe, expect, it } from "vitest";
import {
  RACK_ADD_BIND,
  rackAddBind,
  rackAddBindAt,
  rackBindSymbol,
} from "./graph";

describe("a rack pad's bind", () => {
  it("reads an add bind back to its note, and a bare one to none", () => {
    expect(rackAddBind(rackAddBindAt(40))).toEqual({ note: 40 });
    expect(rackAddBind(RACK_ADD_BIND)).toEqual({ note: null });
    expect(rackAddBind("@rack:BD")).toBeNull();
    expect(rackAddBind("Cutoff")).toBeNull();
  });

  it("reads a pad bind back to its symbol, and leaves every other bind alone", () => {
    // The renderer routes on this: a pad FIRES, where `@mute` and a plain param SET.
    expect(rackBindSymbol("@rack:BD")).toBe("BD");
    expect(rackBindSymbol("@mute")).toBeNull();
    expect(rackBindSymbol("Cutoff")).toBeNull();
    expect(rackBindSymbol(undefined)).toBeNull();
  });
});
