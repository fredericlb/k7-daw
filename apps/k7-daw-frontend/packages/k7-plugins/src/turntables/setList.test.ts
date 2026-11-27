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

describe("a playlist resolved against the workspace's files", () => {
  it("pairs each line with its file, and says whether its stems are beside it", () => {
    const set = resolveSet(
      playlist("records/lemo/danse.flac", "records/aube.wav"),
      FILES,
    );
    expect(set.entries.map((e) => [e.id, e.file, e.stems])).toEqual([
      ["records/lemo/danse.flac", "records/lemo/danse.flac", true],
      ["records/aube.wav", "records/aube.wav", false],
    ]);
  });

  it("keeps a line whose file is not there, with nothing to play", () => {
    const [row] = resolveSet(playlist("records/gone.flac"), FILES).entries;
    expect(row?.file).toBe(null);
    expect(row?.name).toBe("gone");
  });

  it("names a row by its file, and its folder as the artist", () => {
    const [a, b] = resolveSet(
      playlist("records/lemo/danse.flac", "records/aube.wav"),
      FILES,
    ).entries;
    expect([a?.name, a?.artist]).toEqual(["danse", "lemo"]);
    expect([b?.name, b?.artist]).toEqual(["aube", "records"]);
  });

  it("keeps a line that leaves the workspace, playing nothing", () => {
    const [row] = resolveSet(playlist("../x.wav"), FILES).entries;
    expect(row?.file).toBe(null);
  });

  it("drops a record the playlist names twice", () => {
    const set = resolveSet(
      playlist("records/aube.wav", "/records/aube.wav/"),
      FILES,
    );
    expect(set.entries).toHaveLength(1);
  });

  it("plays nothing while the workspace's files are not read", () => {
    const [row] = resolveSet(playlist("records/aube.wav"), null).entries;
    expect(row?.file).toBe(null);
  });

  it("reports a playlist that will not read, and draws no songs", () => {
    const set = resolveSet("items([\n", FILES);
    expect(set.entries).toEqual([]);
    expect(set.issues.length).toBe(1);
    expect(set.absent).toBe(false);
  });

  it("tells a playlist that is MISSING from one that is empty", () => {
    // Three states, three sentences, and the fix differs each time — see ../docs/setList.md.
    const missing = resolveSet(null, FILES);
    expect(missing.entries).toEqual([]);
    expect(missing.absent).toBe(false);
    expect(missing.issues.length).toBe(1);

    const empty = resolveSet(stringify({ items: [] }), FILES);
    expect(empty.entries).toEqual([]);
    expect(empty.absent).toBe(false);
    expect(empty.issues).toEqual([]);

    expect(NO_PLAYLIST.absent).toBe(true);
    expect(NO_PLAYLIST.issues).toEqual([]);
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
