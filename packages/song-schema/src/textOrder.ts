// textOrder.ts — the one order the compile path and the file service sort names in: by Unicode code point.
// It replaces `localeCompare`, whose order is ICU's in node, the locale's in a browser and code points in Rust.
// What a reader gets wrong: this is not UTF-16 order (`<`), which puts U+E000–U+FFFF after an emoji — song-schema/docs/textOrder.md.

/** A UTF-16 unit at or above U+D800, moved so that surrogates (code points above U+FFFF) sort last. */
const lift = (u: number): number => (u < 0xe000 ? u + 0x2000 : u - 0x800);

/** -1, 0 or 1 as `a` sorts before, equal to or after `b` by code point; 0 only for the same string. */
export function byCodePoint(a: string, b: string): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    const x = a.charCodeAt(i);
    const y = b.charCodeAt(i);
    if (x === y) continue;
    if (x >= 0xd800 && y >= 0xd800) return lift(x) < lift(y) ? -1 : 1;
    return x < y ? -1 : 1;
  }
  return a.length === b.length ? 0 : a.length < b.length ? -1 : 1;
}
