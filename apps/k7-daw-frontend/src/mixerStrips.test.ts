// The TypeScript half of mixerStrips is the absent project; the order and the shared card are Rust's (song reads.rs).

import { describe, expect, it } from "vitest";
import { mixerModules } from "./mixerStrips";
import type { WebProject } from "./types";

describe("mixerModules", () => {
  it("reaches the module, and has nothing to draw before the project has loaded", () => {
    const proj = {
      tracks: [
        { id: "vox", mixer: { module: "mx-bus" } },
        { id: "fx" },
        { id: "gtr", mixer: { module: "mx-bus" } },
      ],
    } as unknown as WebProject;
    expect(mixerModules(proj)).toEqual(["mx-bus"]);
    expect(mixerModules(null)).toEqual([]);
  });
});
