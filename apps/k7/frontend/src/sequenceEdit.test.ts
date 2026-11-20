// What a bare name in `sequence edit roll` resolves to.

import { describe, expect, it } from "vitest";
import { resolveSequence } from "./sequenceEdit";

const FILES = [
  "sequences/intro.k7x",
  "sequences/ge.rythm.k7x",
  "clips/intro.k7x",
  "tracks/intro.k7x",
  "modules/audio/verb/module.k7x",
];

describe("resolveSequence", () => {
  it("prefers a SEQUENCE over a clip or a track of the same name", () => {
    // The whole reason this is not the editor's generic resolver: `intro` is three files here, and only one has notes to draw.
    expect(resolveSequence("intro", FILES)).toBe("sequences/intro.k7x");
  });

  it("takes an exact path as itself", () => {
    expect(resolveSequence("clips/intro.k7x", FILES)).toBe("clips/intro.k7x");
  });

  it("still falls back to the rest of the project, so a path can be pasted", () => {
    expect(resolveSequence("verb/module", FILES)).toBe(
      "modules/audio/verb/module.k7x",
    );
  });

  it("matches a dotted sequence id fuzzily", () => {
    expect(resolveSequence("ge.rythm", FILES)).toBe("sequences/ge.rythm.k7x");
  });

  it("says nothing rather than opening an empty grid", () => {
    expect(resolveSequence("zzz nothing", FILES)).toBeNull();
    expect(resolveSequence("", FILES)).toBeNull();
  });
});
