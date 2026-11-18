// songRef.ts — the `@k7-core` ref prefix that names the module library, and the song-ref helpers
// that go with it. A leaf with no imports at all: the browser must not pull `node:os` in with
// the word (mediatekRoot.ts opens one). See packages/song/docs/moduleNamespaces.md.

/** The level the module library is named under — here, so a browser import costs no `homedir()`. */
export const MEDIATEK_REF = "@k7-core";

/** The prefix a SONG ref carries — the médiathèque, then the family. */
export const MEDIATEK_SONG_REF = `${MEDIATEK_REF}/songs`;

/** The STRUCTURE only, since a ref is `@k7-core/` + the id verbatim and spaces are legal. */
export const SONG_REF = new RegExp(
  `^${MEDIATEK_SONG_REF}/(?:[^/\\r\\n]+/)*[^/\\r\\n]+$`,
);

/** True for a well-formed song ref — the shape, never whether the médiathèque has it. */
const isSongRef = (v: unknown): v is string =>
  typeof v === "string" && SONG_REF.test(v);

/** The médiathèque id a ref names (`songs/artist/album/title`), or null when it isn't one. */
export function songIdOf(ref: unknown): string | null {
  return isSongRef(ref) ? ref.slice(MEDIATEK_REF.length + 1) : null;
}

/** The ref for a médiathèque id — the inverse, for a surface that writes the cue sheet. */
export function songRefOf(id: string): string {
  return `${MEDIATEK_REF}/${id.replace(/^\/+|\/+$/g, "")}`;
}

/** The library-relative FOLDER a song ref lives in, with its trailing slash — a song IS its folder. */
export function songPathPrefix(ref: string): string {
  return `${songIdOf(ref) ?? ""}/`;
}
