// The TypeScript half of laneInput is a null check; the walk upstream is Rust's (song reads_ports_misc.rs).

import { describe, expect, it } from "vitest";
import { laneHasInput, laneInput } from "./laneInput";
import type { WebGraph } from "./types";

const graph = (chains: Record<string, unknown>): WebGraph =>
  ({ chains }) as unknown as WebGraph;

describe("laneInput", () => {
  it("reaches the module, and a lane has an input exactly when one comes back", () => {
    const g = graph({
      mic: {
        use: [{ module: "m", monitor: "auto", channel: [3, 4] }],
        out: [{ to: "vox" }],
      },
    });
    expect(laneInput(g, "vox")).toMatchObject({ module: "m", kind: "audio" });
    expect(laneHasInput(g, "vox")).toBe(true);
    expect(laneInput(g, "pad")).toBeNull();
    expect(laneHasInput(g, "pad")).toBe(false);
  });
});
