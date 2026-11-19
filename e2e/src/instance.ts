// instance.ts — what the conformance specs share about the instance under test: its address, who signs in (the
// e2e account, the suite's own account), a small audio file to send and its sidecar fingerprint.
// Every write goes under names a spec made up, and is removed by those names (e2e/docs/conformance.md).
import { createHash } from "node:crypto";
import { type SessionUser, sessionCookies } from "./session.ts";

export const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

/** An account of the suite's own, so its shares and its hunts are nobody's but the suite's. */
export const CONFORMANCE_USER: SessionUser = {
  sub: "conformance",
  name: "Conformance",
  email: "conformance@st0.app",
  groups: [],
};

/** The `cookie` header that signs the page in: a device has one user, whoever a spec names. */
export function signedIn(_user?: SessionUser) {
  return {
    cookie: sessionCookies(base)
      .map((c) => `${c.name}=${c.value}`)
      .join("; "),
  };
}

/** A mono 16-bit 8 kHz WAV, a click every half second: something an analysis can find a tempo in. */
export function clickWav(seconds = 4): Buffer {
  const rate = 8000;
  const frames = rate * seconds;
  const b = Buffer.alloc(44 + frames * 2);
  b.write("RIFF", 0, "ascii");
  b.writeUInt32LE(36 + frames * 2, 4);
  b.write("WAVEfmt ", 8, "ascii");
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24);
  b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36, "ascii");
  b.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++) {
    const inClick = i % (rate / 2) < 80;
    const v = inClick ? Math.round(20000 * Math.sin((i * 2 * Math.PI) / 8)) : 0;
    b.writeInt16LE(v, 44 + i * 2);
  }
  return b;
}

/** A `{file}.cache` sidecar's fingerprint of `bytes`: FNV-1a/64 over the size, the first and the last 64 KB. */
export function fingerprint(bytes: Buffer): string {
  const PRIME = 0x100000001b3n;
  const MASK = (1n << 64n) - 1n;
  let h = 0xcbf29ce484222325n;
  const step = (byte: number) => {
    h = ((h ^ BigInt(byte)) * PRIME) & MASK;
  };
  let s = bytes.length;
  for (let k = 0; k < 8; k++) {
    step(s & 0xff);
    s = Math.floor(s / 256);
  }
  for (const byte of bytes.subarray(0, Math.min(65536, bytes.length)))
    step(byte);
  for (const byte of bytes.subarray(Math.max(0, bytes.length - 65536)))
    step(byte);
  return `${bytes.length}-${h.toString(16).padStart(16, "0")}`;
}

/** The file name an account's per-person store gives `sub`: its slug, then 12 hex of its sha256. */
export function fileFor(sub: string): string {
  const slug = sub.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 40);
  return `${slug}-${createHash("sha256").update(sub).digest("hex").slice(0, 12)}.json`;
}
