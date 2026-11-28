// The TypeScript half of headphonesHint is the call; the two tests are Rust's (song reads_ports_misc.rs).

import { describe, expect, it } from "vitest";
import { givesChannel, hasAudibleInput } from "./headphonesHint";

describe("headphonesHint", () => {
  it("reaches the module", () => {
    expect(givesChannel("node param IN @channel 1,2")).toBe(true);
    const graph = {
      chains: { main: { use: [{ monitor: "on", channel: [1] }] } },
    } as unknown as Parameters<typeof hasAudibleInput>[0];
    expect(hasAudibleInput(graph)).toBe(true);
  });
});
