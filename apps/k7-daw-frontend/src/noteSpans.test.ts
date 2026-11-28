// The TypeScript half of the note spans: the page's document handed over and the peak read by noteLoadAt; the rules are Rust's (song reads_ports.rs).

import { noteLoadAt } from "@studio/ui-chains/noteLoad";
import { describe, expect, it } from "vitest";
import { noteSpans } from "./noteSpans";
import type { WebProject } from "./types";

describe("noteSpans", () => {
  it("reaches the module with the document's tempo, and noteLoadAt agrees with the peak", () => {
    const doc = {
      tempo: 120,
      tracks: [
        {
          id: "m",
          mute: false,
          clips: [
            {
              type: "midi",
              start: 0,
              length: 4,
              notes: [
                { time: 0, duration: 4, key: 60, vel: 0.5 },
                { time: 0, duration: 4, key: 60, vel: 0.5 },
                { time: 2, duration: 1, key: 60 },
              ],
            },
          ],
        },
      ],
    } as unknown as WebProject;
    const out = noteSpans(doc);
    expect(out.m.spans[0]).toEqual({ from: 0, to: 2, vel: 0.5 });
    expect(out.m.peak).toBe(2);
    expect(noteLoadAt(out.m.spans, 1)).toBe(2);
  });

  it("answers an absent document with nothing rather than throwing", () => {
    expect(noteSpans(null)).toEqual({});
  });
});
