import { describe, expect, it } from "vitest";
import {
  placeSubject,
  placeWarnings,
  rawText,
  warningSeen,
  warningSentence,
} from "./engineWarnings";

const w = (message: string, subject = "") => ({ subject, message });

describe("warningSentence", () => {
  it("says a duplicated clip id in plain words", () => {
    expect(
      warningSentence(w("two clips share the id 'a' — the second is 'a#2'")),
    ).toBe(
      "Two clips in this project had the same id; the second one was renamed.",
    );
  });

  it("keeps the raw words, subject first, for a text it does not know", () => {
    const s = warningSentence(w("odd thing", "amp"));
    expect(s).toContain("amp: odd thing");
    expect(rawText(w("odd thing", "amp"))).toBe("amp: odd thing");
  });
});

describe("warningSeen", () => {
  it("says a warning once per scope, however often it comes back", () => {
    const first = warningSeen();
    const dup = w("two clips share the id 'a' — the second is 'a#2'");
    expect(first("song", dup)).toBe(true);
    expect(first("song", { ...dup })).toBe(false);
    expect(first("other", dup)).toBe(true);
    expect(first("song", w("time signature 5/6 cannot be played"))).toBe(true);
    expect(first("song", { ...dup, subject: "x" })).toBe(true);
  });
});

const doc = {
  tracks: [
    { id: "lead", plugins: [{ id: "lead:amp" }, { id: "lead:eq" }] },
    { id: "bass", plugins: [{ id: "bass:comp" }] },
  ],
};

describe("placeSubject", () => {
  it("resolves a module to its track, and a track to itself", () => {
    expect(placeSubject("lead:amp", doc)).toEqual({
      track: "lead",
      module: "lead:amp",
    });
    expect(placeSubject("bass", doc)).toEqual({ track: "bass" });
  });

  it("names nothing for no subject, an unknown one or no project", () => {
    expect(placeSubject("", doc)).toBeNull();
    expect(placeSubject("ghost:amp", doc)).toBeNull();
    expect(placeSubject("lead:amp", null)).toBeNull();
  });
});

describe("placeWarnings", () => {
  it("marks the track and the module, and counts what it placed", () => {
    const placed = placeWarnings(
      { "lead:amp": "no such parameter", "lead:eq": "other", ghost: "lost" },
      doc,
    );
    expect(placed.count).toBe(2);
    expect(placed.byModule).toEqual({
      "lead:amp": "no such parameter",
      "lead:eq": "other",
    });
    expect(placed.byTrack).toEqual({ lead: ["no such parameter", "other"] });
  });

  it("says the automation kind in plain words", () => {
    expect(
      warningSentence(w("automation param not found: cutoff", "lead:amp")),
    ).toBe(
      "The automation on “cutoff” is ignored: this module has no such parameter.",
    );
  });
});
