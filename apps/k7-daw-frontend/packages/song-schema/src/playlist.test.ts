import { parsePlaylist } from "@studio/k7-wasm/schema";
import { K7Document, type K7Scope } from "@studio/k7x/document";
import { describe, expect, it } from "vitest";
import {
  appendPlaylistItem,
  movePlaylistItem,
  playlistStub,
  removePlaylistItem,
} from "./playlist.ts";

/** An edit over a whole file's text, as a caller does through wsEdit: the new text, or null when it refused. */
const edit = (text: string, fn: (doc: K7Scope) => boolean): string | null => {
  const doc = new K7Document(text);
  return fn(doc.at([])) ? doc.toString() : null;
};

describe("playlistStub", () => {
  it("round-trips its own stub, so a file it creates opens on a playlist", () => {
    const stub = playlistStub("répèt du jeudi");
    expect(stub).toBe(
      '// répèt du jeudi\ntitle("répèt du jeudi")\nitems([])\n',
    );
    expect(parsePlaylist(stub)).toEqual({ title: "répèt du jeudi", items: [] });
  });
});

describe("appendPlaylistItem", () => {
  it("adds to the end of the list, a bare song when it names no variant", () => {
    const out = edit('title("Set")\nitems([\n  "Lithium",\n])\n', (d) =>
      appendPlaylistItem(d, { song: "Hells Bells" }),
    );
    expect(out).toBe(
      'title("Set")\nitems([\n  "Lithium",\n  "Hells Bells",\n])\n',
    );
  });

  it("fills the stub's empty list", () => {
    const out = edit(playlistStub("Set"), (d) =>
      appendPlaylistItem(d, { song: "Lithium" }),
    ) as string;
    expect(out.startsWith("// Set\n")).toBe(true);
    expect(parsePlaylist(out)?.items).toEqual([{ song: "Lithium" }]);
  });

  it("writes the variant beside the song", () => {
    const out = edit('items(["Lithium"])\n', (d) =>
      appendPlaylistItem(d, { song: "Nevermind/Lithium", variant: "inst" }),
    ) as string;
    expect(parsePlaylist(out)?.items[1]).toEqual({
      song: "Nevermind/Lithium",
      variant: "inst",
    });
  });

  it("keeps the comments and the order", () => {
    const before = [
      "// la répèt du jeudi",
      'title("Répèt")',
      "items([",
      "  // on ouvre là-dessus",
      '  "Hells Bells",',
      '  "Lithium",',
      "])",
      "",
    ].join("\n");
    const out = edit(before, (d) =>
      appendPlaylistItem(d, { song: "Rock 'n' Roll: Part 2" }),
    ) as string;
    expect(out).toContain("// la répèt du jeudi");
    expect(out).toContain("  // on ouvre là-dessus");
    expect(parsePlaylist(out)?.items.map((i) => i.song)).toEqual([
      "Hells Bells",
      "Lithium",
      "Rock 'n' Roll: Part 2",
    ]);
  });

  it("adds the key when the file has none", () => {
    const out = edit('title("Set")\n', (d) =>
      appendPlaylistItem(d, { song: "Lithium" }),
    ) as string;
    expect(parsePlaylist(out)).toEqual({
      title: "Set",
      items: [{ song: "Lithium" }],
    });
  });

  it("refuses an `items` that is no list rather than guessing", () => {
    expect(
      edit('items("nonsense")\n', (d) => appendPlaylistItem(d, { song: "x" })),
    ).toBeNull();
  });
});

describe("removing and moving a line", () => {
  const SET =
    'items([\n  "a",\n  // the second\n  song("b").variant("v"),\n  "c",\n])\n';

  it("drops one line by its position, and refuses one that is not there", () => {
    const out = edit(SET, (d) => removePlaylistItem(d, 0)) as string;
    expect(parsePlaylist(out)?.items.map((i) => i.song)).toEqual(["b", "c"]);
    expect(edit(SET, (d) => removePlaylistItem(d, 3))).toBeNull();
    expect(edit('title("x")\n', (d) => removePlaylistItem(d, 0))).toBeNull();
  });

  it("moves a line with its comment", () => {
    const out = edit(SET, (d) => movePlaylistItem(d, 1, 2)) as string;
    expect(parsePlaylist(out)?.items.map((i) => i.song)).toEqual([
      "a",
      "c",
      "b",
    ]);
    expect(out).toContain("// the second");
    expect(edit(SET, (d) => movePlaylistItem(d, 0, 5))).toBeNull();
  });
});
