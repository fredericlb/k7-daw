import { describe, expect, it } from "vitest";
import {
  placeSubject,
  placeWarnings,
  rawText,
  warningKind,
  warningSeen,
  warningSentence,
} from "./engineWarnings";

const w = (message: string, subject = "") => ({ subject, message });

describe("warningKind", () => {
  it("reads the engine's own texts", () => {
    expect(
      warningKind(
        "two clips share the id 'track-1_1.1@16' — the second is 'track-1_1.1@16#2'",
      ),
    ).toEqual({ kind: "duplicateId", what: "clips" });
    expect(
      warningKind("two modules share the id 'eq' — the second is 'eq#2'"),
    ).toEqual({ kind: "duplicateId", what: "modules" });
    expect(
      warningKind("time signature 5/6 cannot be played — using 4/4"),
    ).toEqual({ kind: "timeSignature", signature: "5/6" });
    expect(
      warningKind("clip source not found/invalid: C:\\songs\\a\\take.wav"),
    ).toEqual({ kind: "clipMissing", file: "take.wav" });
    expect(
      warningKind(
        "clip source '/home/x/b.wav' is outside the folders this engine reads — ignored",
      ),
    ).toEqual({ kind: "unreadable", file: "b.wav" });
    expect(
      warningKind(
        "record folder '/tmp/r' is outside the folders this engine writes — recording is off",
      ),
    ).toEqual({ kind: "recordFolder" });
    expect(warningKind("could not create plugin 'Diva' (vst3): no")).toEqual({
      kind: "pluginFailed",
      name: "Diva",
    });
    expect(
      warningKind("the audio engine is hung inside a plugin — restart the app"),
    ).toEqual({ kind: "hung" });
    expect(warningKind("an engine job failed: boom")).toEqual({
      kind: "internal",
    });
    expect(warningKind("automation param not found: cutoff")).toEqual({
      kind: "automationParam",
      param: "cutoff",
    });
    expect(warningKind("something new")).toEqual({ kind: "unknown" });
  });
});

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
