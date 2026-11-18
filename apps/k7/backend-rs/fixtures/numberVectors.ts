// numberVectors.ts — JavaScript's own toFixed, String(number), Math.round and the room's textHash over
// doubles and strings drawn from a PRNG k7-js's tests replay bit for bit; digests, so millions fit in a file.
// What reads wrong: the PRNG and the line format are a contract with crates/k7-js/tests/all/number.rs.

import { createHash } from "node:crypto";

/** A 53-bit hash of a text, as hex: the one a whole-text op names the text it replaced by. */
function textHash(text: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 =
    Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^
    Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 =
    Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^
    Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(16);
}

const MASK = (1n << 64n) - 1n;

/** splitmix64, the generator both sides replay. */
export function splitmix(seed: bigint): () => bigint {
  let s = seed & MASK;
  return () => {
    s = (s + 0x9e3779b97f4a7c15n) & MASK;
    let z = s;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
    return z ^ (z >> 31n);
  };
}

const view = new DataView(new ArrayBuffer(8));
const fromBits = (b: bigint): number => {
  view.setBigUint64(0, b);
  return view.getFloat64(0);
};
const toBits = (x: number): bigint => {
  view.setFloat64(0, x);
  return view.getBigUint64(0);
};
const unit = (r: bigint): number => Number(r >> 11n) / 2 ** 53;

/** The i-th kind of double as its bits: raw bits, unit interval, exact ties, hundredths, any magnitude. */
export function drawBits(next: () => bigint): bigint {
  const kind = Number(next() % 5n);
  const neg = (next() & 1n) === 1n ? -1 : 1;
  const r = next();
  // Bits, never a number: V8 quiets a NaN's payload in some tiers, so a NaN never leaves as a double.
  if (kind === 0) return r;
  if (kind === 1) return toBits(neg * unit(r));
  if (kind === 2)
    return toBits(
      (neg * Number(r % 1048576n)) / 2 ** Number(1n + ((r >> 40n) % 12n)),
    );
  if (kind === 3) return toBits((neg * Number(r % 30000n)) / 100);
  // A parsed literal, never `10 **`: pow is not correctly rounded in every engine.
  return toBits(neg * unit(r) * Number(`1e${Number((r >> 50n) % 61n) - 30}`));
}

/** One double's line: its bits, then each format JavaScript gives it. */
export function numberLine(bits: bigint): string {
  const x = fromBits(bits);
  const fixed = [0, 2, 4, 6].map((d) => x.toFixed(d)).join(" ");
  return `${bits.toString(16)} ${fixed} ${String(x)} ${String(Math.round(x))}\n`;
}

/** A string of 0 to 40 UTF-16 units, surrogates only in pairs, from the generator. */
export function drawText(next: () => bigint): string {
  const len = Number(next() % 41n);
  let s = "";
  while (s.length < len) {
    const r = next();
    const kind = Number(r % 4n);
    const v = Number((r >> 8n) % 0x10000n);
    if (kind === 0) s += String.fromCharCode(32 + (v % 95));
    else if (kind === 1)
      s += String.fromCodePoint(0x10000 + Number((r >> 8n) % 0x100000n));
    else s += String.fromCharCode(v >= 0xd800 && v <= 0xdfff ? v - 0x800 : v);
  }
  return s;
}

const BATCH = 100_000;

/** Digests of `batches` batches of number lines, and of text-hash lines. */
export function numberVectors(batches = 10) {
  const next = splitmix(0x6b37n);
  const numbers: string[] = [];
  for (let b = 0; b < batches; b++) {
    const h = createHash("sha256");
    for (let i = 0; i < BATCH; i++) h.update(numberLine(drawBits(next)));
    numbers.push(h.digest("hex"));
  }
  const texts = splitmix(0x7e47n);
  const h = createHash("sha256");
  for (let i = 0; i < BATCH; i++) h.update(`${textHash(drawText(texts))}\n`);
  const edges = [
    0,
    -0,
    0.5,
    1.5,
    2.5,
    -0.5,
    -2.5,
    0.125,
    0.375,
    1.005,
    1.255,
    10.235,
    0.000001,
    1e-7,
    123.456,
    1e20,
    1e21,
    -1e21,
    1.7976931348623157e308,
    5e-324,
    Number.NaN,
    Infinity,
    -Infinity,
    0.49999999999999994,
    4503599627370495.5,
    9007199254740994,
    127.98,
    195.025,
    0.00005,
    0.00015,
  ].map((x) => numberLine(toBits(x)));
  return { batch: BATCH, numbers, textHashes: h.digest("hex"), edges };
}
