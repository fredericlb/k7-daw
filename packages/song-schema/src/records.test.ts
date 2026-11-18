// What a set list's line means against the workspace's files: the file it names, whether it is
// there, whether its stems are, and the drag a row carries.

import { describe, expect, it } from "vitest";
import {
  audioFilesOf,
  isRecordRef,
  packRecordDrag,
  parseRecordDrag,
  recordFolder,
  recordMatches,
  recordName,
  recordPathOf,
  recordStemsDir,
  resolveRecords,
} from "./records";

const files = new Set([
  "records/hells-bells/hells-bells.flac",
  "records/hells-bells/stems/drums.wav",
  "records/hells-bells/stems/bass.wav",
  "records/hells-bells/stems/other.wav",
  "records/hells-bells/stems/vocals.wav",
  "records/thunder.mp3",
  "records/stems/drums.wav",
]);

describe("recordPathOf", () => {
  it("keeps a workspace path, its slashes trimmed", () => {
    expect(recordPathOf("/records/thunder.mp3/")).toBe("records/thunder.mp3");
  });
  it("refuses one that leaves the workspace or is no path", () => {
    expect(recordPathOf("../x.wav")).toBeNull();
    expect(recordPathOf("a/./b.wav")).toBeNull();
    expect(recordPathOf("a\\b.wav")).toBeNull();
    expect(recordPathOf("  ")).toBeNull();
  });
  it("is the ref rule of the cue sheet", () => {
    expect(isRecordRef("records/thunder.mp3")).toBe(true);
    expect(isRecordRef("/records/thunder.mp3")).toBe(false);
    expect(isRecordRef("@k7-core/songs/a/b/c")).toBe(false);
  });
});

describe("resolveRecords", () => {
  it("says which lines are there, and which have their four stems", () => {
    const rows = resolveRecords(
      [
        { song: "records/hells-bells/hells-bells.flac" },
        { song: "records/thunder.mp3" },
        { song: "records/gone.wav" },
        { song: "../escape.wav" },
      ],
      files,
    );
    expect(rows.map((r) => [r.path, r.present, r.stems])).toEqual([
      ["records/hells-bells/hells-bells.flac", true, true],
      ["records/thunder.mp3", true, false],
      ["records/gone.wav", false, false],
      [null, false, false],
    ]);
  });
  it("names no file present while the tree is not read", () => {
    const [row] = resolveRecords([{ song: "records/thunder.mp3" }], null);
    expect(row?.present).toBe(false);
  });
  it("does not take a non-audio file for a record", () => {
    const [row] = resolveRecords(
      [{ song: "notes.txt" }],
      new Set(["notes.txt"]),
    );
    expect(row?.present).toBe(false);
  });
});

describe("the names a row shows", () => {
  it("is the file's name and its folder", () => {
    expect(recordName("records/hells-bells/Hells Bells.flac")).toBe(
      "Hells Bells",
    );
    expect(recordFolder("records/hells-bells/x.flac")).toBe("hells-bells");
    expect(recordFolder("x.flac")).toBeNull();
    expect(recordStemsDir("records/x.flac")).toBe("records/stems/");
    expect(recordStemsDir("x.flac")).toBe("stems/");
  });
  it("matches every word against the path", () => {
    expect(recordMatches("records/AC-DC/hells.flac", "acdc")).toBe(false);
    expect(recordMatches("records/AC-DC/hells.flac", "ac-dc hell")).toBe(true);
  });
});

describe("audioFilesOf", () => {
  it("flattens a tree to its audio files, leaving every stems folder out", () => {
    const tree = [
      {
        path: "records",
        dir: true,
        children: [
          { path: "records/a.flac", dir: false },
          { path: "records/notes.txt", dir: false },
          {
            path: "records/stems",
            dir: true,
            children: [{ path: "records/stems/drums.wav", dir: false }],
          },
        ],
      },
    ];
    expect(audioFilesOf(tree, false)).toEqual(["records/a.flac"]);
    expect(audioFilesOf(tree, true)).toEqual([
      "records/a.flac",
      "records/stems/drums.wav",
    ]);
  });
});

describe("the drag a row carries", () => {
  it("round-trips, and refuses anything else", () => {
    expect(parseRecordDrag(packRecordDrag({ record: "a/b.wav" }))).toEqual({
      record: "a/b.wav",
    });
    expect(parseRecordDrag('{"song":"a"}')).toBeNull();
    expect(parseRecordDrag("not json")).toBeNull();
  });
});
