// text.test.ts — the text writers reach the module: a lane edit, a cue write, a playlist append and the facade's error and shape.
// The rules (every exact text, refusal and short form) are Rust's: k7-wasm's tests_text_*.rs, song-edit's text_writers.rs.

import { describe, expect, it } from "vitest";
import { parse } from "./k7x/index.ts";
import {
  editAssembly,
  editMix,
  editPlaylist,
  editVideo,
  expandSugar,
  listVideoPlacements,
  mixCues,
  parseLibraryPins,
  playlistStub,
  stringifyLibraryPins,
} from "./text.ts";

describe("the text writers", () => {
  it("edits a picture lane, a refusal being a false and the text kept", () => {
    const lane = 'events([ref("a").start(0).length(4)])\n';
    const split = editVideo(lane, [
      { op: "splitAt", index: 0, cut: 1 },
      { op: "removeAt", index: 9 },
    ]);
    expect(split.results).toEqual([true, false]);
    expect(listVideoPlacements(split.text).map((p) => p.start)).toEqual([0, 1]);
    const replay = split.changes.reduce(
      (t, c) => t.slice(0, c.start) + c.insert + t.slice(c.end),
      lane,
    );
    expect(replay).toBe(split.text);
  });

  it("writes a cue, a playlist line and an assembly param over a file's text", () => {
    const cued = editMix('cues(["records/a.flac"])\n', [
      { op: "withMarkers", ref: "records/a.flac", markers: [1, null, 3] },
    ]);
    expect(mixCues(cued.text).markers).toEqual({
      "records/a.flac": [1, null, 3],
    });
    const list = editPlaylist(playlistStub("Set"), [
      { op: "append", song: "Lithium" },
    ]);
    expect(parse(list.text).items).toEqual(["Lithium"]);
    const asm = editAssembly('label("Box")\n', [
      { op: "setParam", name: "T", node: "n", param: "P" },
    ]);
    expect(parse(asm.text).params).toEqual({ T: { node: "n", param: "P" } });
  });

  it("throws the sentence of a text that does not read, and reads pins and short forms", () => {
    expect(() => editMix("cues([", [])).toThrow();
    const pins = parseLibraryPins('"k7-core"(7)\ndrums("draft")\n');
    expect(stringifyLibraryPins(pins)).toBe('drums("draft")\nk7-core(7)\n');
    expect(expandSugar("song/sequences/m.k7x", { notes: "c4" })).toEqual({
      notes: [{ t: 0, d: 1, k: 60 }],
    });
  });
});
