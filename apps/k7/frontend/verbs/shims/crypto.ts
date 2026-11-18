// crypto.ts — `node:crypto` in the verbs' Worker: the synchronous sha1 and sha256 digests song hashes with, and randomUUID.
// What a reader gets wrong: the digests must equal node's bit for bit — they name freeze renders and parse-cache keys.

type Bytes = string | ArrayBufferView;

const asBytes = (d: Bytes): Uint8Array =>
  typeof d === "string"
    ? new TextEncoder().encode(d)
    : new Uint8Array(d.buffer, d.byteOffset, d.byteLength);

const K256 = Uint32Array.from(
  Array.from({ length: 64 }, (_, i) => {
    let n = 2;
    for (let c = 0; c <= i; n++) {
      let prime = true;
      for (let d = 2; d * d <= n; d++) if (n % d === 0) prime = false;
      if (prime) c++;
    }
    return Math.floor((Math.cbrt(n - 1) % 1) * 2 ** 32);
  }),
);

const H256 = Uint32Array.from(
  Array.from({ length: 8 }, (_, i) => {
    const primes = [2, 3, 5, 7, 11, 13, 17, 19];
    return Math.floor((Math.sqrt(primes[i] as number) % 1) * 2 ** 32);
  }),
);

const rotl = (x: number, n: number): number => (x << n) | (x >>> (32 - n));
const rotr = (x: number, n: number): number => (x >>> n) | (x << (32 - n));

/** Pads a message to a multiple of 64 bytes with its bit length in big-endian. */
function padded(data: Uint8Array): Uint8Array {
  const total = Math.ceil((data.length + 9) / 64) * 64;
  const out = new Uint8Array(total);
  out.set(data);
  out[data.length] = 0x80;
  const view = new DataView(out.buffer);
  view.setUint32(total - 8, Math.floor((data.length * 8) / 2 ** 32));
  view.setUint32(total - 4, (data.length * 8) >>> 0);
  return out;
}

function sha256(data: Uint8Array): Uint8Array {
  const h = Uint32Array.from(H256);
  const w = new Uint32Array(64);
  const msg = padded(data);
  const view = new DataView(msg.buffer);
  for (let off = 0; off < msg.length; off += 64) {
    for (let i = 0; i < 16; i++) w[i] = view.getUint32(off + i * 4);
    for (let i = 16; i < 64; i++) {
      const a = w[i - 15] as number;
      const b = w[i - 2] as number;
      const s0 = rotr(a, 7) ^ rotr(a, 18) ^ (a >>> 3);
      const s1 = rotr(b, 17) ^ rotr(b, 19) ^ (b >>> 10);
      w[i] = ((w[i - 16] as number) + s0 + (w[i - 7] as number) + s1) | 0;
    }
    let [a, b, c, d, e, f, g, hh] = h as unknown as number[] as [
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
    ];
    for (let i = 0; i < 64; i++) {
      const t1 =
        (hh +
          (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) +
          ((e & f) ^ (~e & g)) +
          (K256[i] as number) +
          (w[i] as number)) |
        0;
      const t2 =
        ((rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) +
          ((a & b) ^ (a & c) ^ (b & c))) |
        0;
      hh = g;
      g = f;
      f = e;
      e = (d + t1) | 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) | 0;
    }
    const next = [a, b, c, d, e, f, g, hh];
    for (let i = 0; i < 8; i++)
      h[i] = ((h[i] as number) + (next[i] as number)) | 0;
  }
  const out = new Uint8Array(32);
  const o = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) o.setUint32(i * 4, h[i] as number);
  return out;
}

function sha1(data: Uint8Array): Uint8Array {
  const h = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
  const w = new Uint32Array(80);
  const msg = padded(data);
  const view = new DataView(msg.buffer);
  for (let off = 0; off < msg.length; off += 64) {
    for (let i = 0; i < 16; i++) w[i] = view.getUint32(off + i * 4);
    for (let i = 16; i < 80; i++)
      w[i] = rotl(
        (w[i - 3] as number) ^
          (w[i - 8] as number) ^
          (w[i - 14] as number) ^
          (w[i - 16] as number),
        1,
      );
    let [a, b, c, d, e] = h as [number, number, number, number, number];
    for (let i = 0; i < 80; i++) {
      const [f, k] =
        i < 20
          ? [(b & c) | (~b & d), 0x5a827999]
          : i < 40
            ? [b ^ c ^ d, 0x6ed9eba1]
            : i < 60
              ? [(b & c) | (b & d) | (c & d), 0x8f1bbcdc]
              : [b ^ c ^ d, 0xca62c1d6];
      const t =
        (rotl(a, 5) + (f as number) + e + (k as number) + (w[i] as number)) | 0;
      e = d;
      d = c;
      c = rotl(b, 30);
      b = a;
      a = t;
    }
    const next = [a, b, c, d, e];
    for (let i = 0; i < 5; i++)
      h[i] = ((h[i] as number) + (next[i] as number)) | 0;
  }
  const out = new Uint8Array(20);
  const o = new DataView(out.buffer);
  for (let i = 0; i < 5; i++) o.setUint32(i * 4, h[i] as number);
  return out;
}

const HASHES: Record<string, (d: Uint8Array) => Uint8Array> = { sha1, sha256 };

export interface Hash {
  update(data: Bytes, inputEncoding?: "utf8"): Hash;
  digest(encoding: "hex" | "base64"): string;
}

export function createHash(algorithm: string): Hash {
  const hash = HASHES[algorithm];
  if (!hash) throw new Error(`createHash: ${algorithm} is not supported`);
  const parts: Uint8Array[] = [];
  const self: Hash = {
    update(data, inputEncoding) {
      // Node reads a string in the named encoding; only utf8 is held here, so nothing hashes differently without saying so.
      if (inputEncoding !== undefined && inputEncoding !== "utf8")
        throw new Error(
          `update: the ${inputEncoding} input encoding is not supported`,
        );
      parts.push(asBytes(data));
      return self;
    },
    digest(encoding) {
      if (encoding !== "hex" && encoding !== "base64")
        throw new Error(
          `digest: the ${String(encoding)} encoding is not supported`,
        );
      const all = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
      let at = 0;
      for (const p of parts) {
        all.set(p, at);
        at += p.length;
      }
      const out = hash(all);
      return encoding === "hex"
        ? Array.from(out, (b) => b.toString(16).padStart(2, "0")).join("")
        : btoa(String.fromCharCode(...out));
    },
  };
  return self;
}

/** A v4 uuid; `crypto.randomUUID` exists only in a secure context, a page on a LAN address has `getRandomValues` alone. */
export function randomUUID(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = ((b[6] as number) & 0x0f) | 0x40;
  b[8] = ((b[8] as number) & 0x3f) | 0x80;
  const h = Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}
