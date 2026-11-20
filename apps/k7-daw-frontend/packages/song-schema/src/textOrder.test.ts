import { describe, expect, it } from "vitest";
import { byCodePoint } from "./textOrder";

/** UTF-8 byte order, which is code-point order and what Rust's `str` comparison gives. */
const utf8 = (a: string, b: string): number =>
  Math.sign(Buffer.compare(Buffer.from(a), Buffer.from(b)));

const SAMPLES = [
  "",
  "a",
  "A",
  "b",
  "B",
  "_references",
  "-x",
  "10",
  "9",
  "ab",
  "a b",
  "é",
  "é",
  "z",
  "",
  "�",
  "\u{1F3B5}",
  "\u{1F3B5}a",
  "\u{10000}",
  "Bass",
  "bass",
  "drums",
];

describe("byCodePoint", () => {
  it("puts upper case before lower case and symbols where their code point is", () => {
    expect(
      ["drums", "Bass", "_x", "bass", "-y", "9"].sort(byCodePoint),
    ).toEqual(["-y", "9", "Bass", "_x", "bass", "drums"]);
  });

  it("sorts a code point above U+FFFF after U+E000–U+FFFF, unlike UTF-16 order", () => {
    expect(byCodePoint("\u{1F3B5}", "�")).toBe(1);
    expect("\u{1F3B5}" < "�").toBe(true);
  });

  it("orders every pair as UTF-8 bytes do, and is 0 only for the same string", () => {
    for (const a of SAMPLES)
      for (const b of SAMPLES) {
        expect(byCodePoint(a, b)).toBe(utf8(a, b));
        expect(byCodePoint(a, b)).toBe(-byCodePoint(b, a) || 0);
      }
  });
});
