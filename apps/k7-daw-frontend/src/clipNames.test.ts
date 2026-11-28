import { describe, expect, it } from "vitest";
import { type ClipNameWords, clipShownName, laneNamer } from "./clipNames";

const words: ClipNameWords = {
  take: ({ lane, bar }) => `${lane} take · bar ${bar}`,
  sequence: ({ lane, n }) => `${lane} sequence ${n}`,
};
const lanes = laneNamer([
  { id: "track-1", name: "ga.rythm" },
  { id: "track-7", name: "da.drums" },
  { id: "my lane", name: "" },
]);

describe("clipShownName", () => {
  it("names a take by its lane's label and its bar", () => {
    expect(clipShownName("track-1_9.1_261030T172747", lanes, words)).toBe(
      "rythm take · bar 9",
    );
    expect(clipShownName("track-1_1.1_261030T172747-2@32", lanes, words)).toBe(
      "rythm take · bar 1 (2)",
    );
  });
  it("names a take whose lane was renamed since by the lane it sits on", () => {
    expect(
      clipShownName("track-2_1.1_261030T173449", lanes, words, "marie1"),
    ).toBe("marie1 take · bar 1");
    // A token that still names a lane wins over the lane the clip was moved to.
    expect(
      clipShownName("track-1_9.1_261030T172747", lanes, words, "marie1"),
    ).toBe("rythm take · bar 9");
  });
  it("names a sequence by its lane's label and number", () => {
    expect(clipShownName("track-7-seq-1", lanes, words)).toBe(
      "drums sequence 1",
    );
  });
  it("finds a lane whose id had to be made filename-safe, and keeps an unknown token", () => {
    expect(clipShownName("my-lane_2.1_261030T172747", lanes, words)).toBe(
      "my lane take · bar 2",
    );
    expect(clipShownName("gone_2.1_261030T172747", lanes, words)).toBe(
      "gone take · bar 2",
    );
  });
  it("shows an authored name as written", () => {
    expect(clipShownName("chorus-gtr", lanes, words)).toBe("chorus-gtr");
  });
});
