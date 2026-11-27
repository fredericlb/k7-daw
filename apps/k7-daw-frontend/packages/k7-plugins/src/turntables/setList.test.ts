import { stringify } from "@studio/k7-wasm/k7x";
import { describe, expect, it } from "vitest";
import {
  browseSongs,
  entryForDrag,
  NO_PLAYLIST,
  resolveSet,
  type SetEntry,
} from "./setList";

// The browser's list comes from the PLAYLIST the surface names, its lines the workspace's audio files — see ../docs/setList.md.

const FILES = new Set([
  "records/lemo/danse.flac",
  "records/lemo/stems/drums.wav",
  "records/lemo/stems/bass.wav",
  "records/lemo/stems/other.wav",
  "records/lemo/stems/vocals.wav",
  "records/aube.wav",
]);

/** A playlist naming these refs, in this order. The format is song-edit's playlist writer (@studio/k7-wasm/text). */
const playlist = (...refs: string[]) =>
  stringify({ title: "Set", items: refs });

describe("the song browser's list", () => {
  const song = (artist: string, name: string): SetEntry => ({
    ref: `records/${artist}/${name}.flac`,
    id: `records/${artist}/${name}.flac`,
    name,
    artist,
    file: `records/${artist}/${name}.flac`,
    stems: false,
  });

  const LIBRARY = [
    song("Daft Punk", "Around the world"),
    song("Alizée", "Moi Lolita"),
    song("Alain Bashung", "La nuit je mens"),
    song("Alain Bashung", "Ma petite entreprise"),
    song("& Feduk", "Rosé"),
  ];

  it("sorts by artist, then by title", () => {
    expect(
      browseSongs(LIBRARY, "").map((s) => `${s.artist} — ${s.name}`),
    ).toEqual([
      "& Feduk — Rosé",
      "Alain Bashung — La nuit je mens",
      "Alain Bashung — Ma petite entreprise",
      "Alizée — Moi Lolita",
      "Daft Punk — Around the world",
    ]);
  });

  it("files an accented artist where a reader looks for it", () => {
    // `Alizée` between `Alain` and `Daft` — comparing code points files every accent after Z.
    const order = browseSongs(LIBRARY, "").map((s) => s.artist);
    expect(order.indexOf("Alizée")).toBeGreaterThan(
      order.indexOf("Alain Bashung"),
    );
    expect(order.indexOf("Alizée")).toBeLessThan(order.indexOf("Daft Punk"));
  });

  it("finds an accented name typed without accents", () => {
    // What decides whether the search is usable at a desk: nobody composes an é mid-set.
    expect(browseSongs(LIBRARY, "alizee").map((s) => s.name)).toEqual([
      "Moi Lolita",
    ]);
    expect(browseSongs(LIBRARY, "rose").map((s) => s.name)).toEqual(["Rosé"]);
  });

  it("matches terms in ANY order, across artist and title", () => {
    // `bashung nuit` without knowing which field holds which word — neither naive search does this.
    expect(browseSongs(LIBRARY, "bashung nuit").map((s) => s.name)).toEqual([
      "La nuit je mens",
    ]);
    expect(browseSongs(LIBRARY, "nuit bashung").map((s) => s.name)).toEqual([
      "La nuit je mens",
    ]);
  });

  it("filters the WHOLE list, not the page", () => {
    expect(browseSongs(LIBRARY, "bashung")).toHaveLength(2);
    expect(browseSongs(LIBRARY, "  ")).toHaveLength(LIBRARY.length);
    expect(browseSongs(LIBRARY, "zzz")).toEqual([]);
  });

  it("leaves the caller's array alone", () => {
    // `sort` mutates in place, and the set list is state: it would reorder on every keystroke.
    const before = LIBRARY.map((s) => s.name);
    browseSongs(LIBRARY, "");
    expect(LIBRARY.map((s) => s.name)).toEqual(before);
  });
});

describe("the row a dropped playlist line means", () => {
  const set = resolveSet(playlist("records/aube.wav"), FILES);

  it("finds the row by its record", () => {
    expect(entryForDrag(set.entries, { record: "records/aube.wav" })?.id).toBe(
      "records/aube.wav",
    );
  });

  it("forgives the slashes a line may carry", () => {
    expect(
      entryForDrag(set.entries, { record: "/records/aube.wav/" })?.id,
    ).toBe("records/aube.wav");
  });

  it("says nothing for a record this set list does not carry", () => {
    // The panel may show ANOTHER playlist than the surface names — the view turns this into a sentence.
    expect(entryForDrag(set.entries, { record: "x/y.wav" })).toBeUndefined();
  });
});

describe("a playlist's trouble, in words", () => {
  it("turns each issue key into the sentence the page shows", () => {
    // The rule that says WHICH trouble is Rust's (tests/set_list.rs); the words are the page's.
    expect(resolveSet(null, FILES).issues).toHaveLength(1);
    expect(resolveSet("items([\n", FILES).issues).toHaveLength(1);
    expect(resolveSet(playlist("records/aube.wav"), FILES).issues).toEqual([]);
    expect(NO_PLAYLIST.absent).toBe(true);
  });
});
