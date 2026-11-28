// The TypeScript half of midiIn: the lanes handed back as a Set; the graph rules are Rust's (song reads_ports.rs).

import { describe, expect, it } from "vitest";
import { midiInDeviceFor, midiInLanes, midiInPlayed } from "./midiIn";
import type { WebGraph } from "./types";

const g = {
  tracks: ["lead"],
  chains: {
    keys: {
      from: "keys",
      use: [{ module: "keys", device: "KeyStep 37" }],
      out: [{ to: "piano#midi" }, { to: "lead" }],
    },
    piano: { use: [{ module: "piano" }], out: [{ to: "MIX" }] },
  },
} as unknown as WebGraph;

describe("midiIn", () => {
  it("reaches the module and hands the lanes back as a Set", () => {
    expect(midiInDeviceFor(g, "lead")).toBe("KeyStep 37");
    expect(midiInLanes(g)).toBeInstanceOf(Set);
    expect(midiInLanes(g).has("lead")).toBe(true);
    expect(midiInPlayed(g)).toEqual([{ track: "piano", device: "KeyStep 37" }]);
  });
});
