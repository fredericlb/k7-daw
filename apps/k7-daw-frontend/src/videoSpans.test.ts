// The TypeScript half of the picture spans: an open end read back as an infinity; the rules are Rust's (song reads_ports.rs).

import type { WebVideoProject } from "@studio/web-protocol";
import { describe, expect, it } from "vitest";
import { videoSpans } from "./videoSpans";

const doc = (videoTracks: unknown[]) =>
  ({ videoTracks }) as unknown as WebVideoProject;

describe("videoSpans", () => {
  it("reads an open end as an infinity and keys the spans by lane", () => {
    const spans = videoSpans(
      doc([
        { id: "titles", mute: false, clips: [], shapes: [{}] },
        {
          id: "cam",
          mute: false,
          clips: [{ start: 0, length: 2 }],
          shapes: [],
        },
      ]),
    );
    expect(spans).toEqual({
      titles: [{ from: 0, to: Number.POSITIVE_INFINITY }],
      cam: [{ from: 0, to: 2 }],
    });
  });

  it("answers an absent document with nothing", () => {
    expect(videoSpans(null)).toEqual({});
  });
});
