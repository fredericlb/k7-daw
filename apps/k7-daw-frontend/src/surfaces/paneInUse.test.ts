// The wiring of the pane rules: a Set in, the lists across, the answers back. The rules are tested in the chains crate
// (apps/k7-daw-wasm/chains/src/pane_in_use/tests.rs).

import { describe, expect, it } from "vitest";
import {
  inUseBy,
  type MainSurfaceIds,
  nextFree,
  sameScreens,
} from "./paneInUse";

const ids: MainSurfaceIds = {
  timeline: "arr",
  turntables: "desk",
  viewport: null,
};

describe("paneInUse", () => {
  it("maps a remote's surface to the pane it is on, and steps past a blocked one", () => {
    const screens = [{ surface: "desk", device: "iPad", since: 1 }];
    expect(inUseBy(screens, ids)).toEqual({ turntables: "iPad" });
    const order = ["timeline", "turntables", "chains"];
    expect(
      nextFree(order, "timeline", new Set(["timeline", "turntables"])),
    ).toBe("chains");
    expect(nextFree(["a", "b"], "a", new Set(["a", "b"]))).toBeNull();
    expect(sameScreens(screens, [{ ...screens[0] }])).toBe(true);
  });
});
