// The TypeScript half of the loop marks: what is drawn per zoom; the passes and the axes are Rust's (song reads_ports.rs).

import { describe, expect, it } from "vitest";
import {
  clipLoop,
  placedRepeats,
  playedZone,
  visibleRepeats,
} from "./clipLoop";
import type { WebClip } from "./types";

const clip = (over: Partial<WebClip>): WebClip =>
  ({
    id: "c",
    type: "midi",
    name: "c",
    start: 0,
    length: 0,
    notes: [],
    ...over,
  }) as WebClip;

describe("clipLoop", () => {
  it("reaches the module", () => {
    const l = clipLoop(clip({ length: 80, loopStart: 0, loopEnd: 8 }));
    expect(l).toMatchObject({ from: 0, to: 8, win: 8, passes: 10 });
    expect(clipLoop(clip({}))).toBeNull();
    expect(playedZone(clip({ length: 4 }))).toEqual({ from: 0, to: 4 });
  });

  it("draws the repeats only when a pass is wide enough to tell apart", () => {
    const l = clipLoop(clip({ length: 80, loopStart: 0, loopEnd: 8 }));
    if (!l) throw new Error("fixture has a loop");
    expect(visibleRepeats(l, 24)).toHaveLength(9);
    expect(visibleRepeats(l, 0.5)).toEqual([]);
    expect(placedRepeats(l, 0.5)).toEqual([]);
  });

  it("counts a placed repeat from the EVENT's start, not from the material's", () => {
    const l = clipLoop(
      clip({ type: "midi", length: 12, loopStart: 8, loopEnd: 12 }),
    );
    if (!l) throw new Error("fixture has a loop");
    expect(visibleRepeats(l, 24)).toEqual([12, 16]);
    expect(placedRepeats(l, 24)).toEqual([4, 8]);
  });

  it("marks nothing placed for a window that plays once", () => {
    const l = clipLoop(clip({ length: 4, loopStart: 0, loopEnd: 4 }));
    if (!l) throw new Error("fixture has a loop");
    expect(placedRepeats(l, 24)).toEqual([]);
  });
});
