// An event's comp: which material plays over which span, and how a new span lands over the rest.

import { describe, expect, it } from "vitest";
import { compEnd, compPieces, seamCrossfades, setCompSpan } from "./takes";

const base = { ref: "gtr", start: 16, offset: 0, length: 8 };

describe("compPieces", () => {
  it("is the event's own material when nothing is comped", () => {
    expect(compPieces(base, [], [])).toEqual([base]);
  });

  it("punches a take into the middle, reading it from where the span starts in it", () => {
    const pieces = compPieces(
      base,
      [{ ref: "t2", start: 16 }],
      [{ take: "t2", from: 18, to: 20 }],
    );
    expect(pieces).toEqual([
      { ref: "gtr", start: 16, offset: 0, length: 2 },
      { ref: "t2", start: 18, offset: 2, length: 2 },
      { ref: "gtr", start: 20, offset: 4, length: 4 },
    ]);
  });

  it("extends the event past its end, and plays nothing where a span is silence", () => {
    const pieces = compPieces(
      base,
      [{ ref: "t2", start: 22 }],
      [
        { take: "t2", from: 22, to: 28 },
        { take: "", from: 17, to: 18 },
      ],
    );
    expect(pieces).toEqual([
      { ref: "gtr", start: 16, offset: 0, length: 1 },
      { ref: "gtr", start: 18, offset: 2, length: 4 },
      { ref: "t2", start: 22, offset: 0, length: 6 },
    ]);
    expect(compEnd(base, [{ take: "t2", from: 22, to: 28 }])).toBe(28);
  });

  it("never reads a take before its first beat", () => {
    const pieces = compPieces(
      base,
      [{ ref: "t2", start: 19 }],
      [{ take: "t2", from: 17, to: 21 }],
    );
    expect(pieces.find((p) => p.ref === "t2")).toEqual({
      ref: "t2",
      start: 19,
      offset: 0,
      length: 2,
    });
  });
});

describe("setCompSpan", () => {
  it("takes what the new span covers from every other, and writes nothing for the base", () => {
    const comp = [{ take: "t2", from: 16, to: 24 }];
    expect(setCompSpan(comp, { take: "t3", from: 18, to: 20 }, "gtr")).toEqual([
      { take: "t2", from: 16, to: 18 },
      { take: "t3", from: 18, to: 20 },
      { take: "t2", from: 20, to: 24 },
    ]);
    expect(setCompSpan(comp, { take: "gtr", from: 16, to: 20 }, "gtr")).toEqual(
      [{ take: "t2", from: 20, to: 24 }],
    );
  });
});

describe("seamCrossfades", () => {
  it("overlaps the two pieces at a seam by the crossfade, centred, within their material", () => {
    const pieces = compPieces(
      base,
      [{ ref: "t2", start: 16 }],
      [{ take: "t2", from: 18, to: 20 }],
    );
    const faded = seamCrossfades(pieces, [{ at: 18, length: 1 }], () => 12);
    expect(faded[0]).toMatchObject({
      ref: "gtr",
      start: 16,
      length: 2.5,
      fadeOut: 1,
    });
    expect(faded[1]).toMatchObject({
      ref: "t2",
      start: 17.5,
      offset: 1.5,
      fadeIn: 1,
    });
  });

  it("shrinks a crossfade to what a take has before the seam", () => {
    const pieces = compPieces(
      base,
      [{ ref: "t2", start: 18 }],
      [{ take: "t2", from: 18, to: 20 }],
    );
    // t2 starts AT the seam: nothing of it to fade in early, so no crossfade at all.
    const faded = seamCrossfades(pieces, [{ at: 18, length: 1 }], () => 12);
    expect(faded[1].fadeIn).toBeUndefined();
  });
});
