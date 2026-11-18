// globals.ts — the node globals the verbs read and a browser Worker lacks: `process` (env, platform, cwd) and a small `Buffer`.
// What a reader gets wrong: under node (the runner, vitest) nothing is installed, the real ones stay.

import { current } from "./shims/current.ts";

/** Installs `process` and `Buffer` where the runtime has none. */
export function installGlobals(): void {
  const g = globalThis as Record<string, unknown>;
  g.process ??= {
    env: {},
    platform: "linux",
    pid: 1,
    argv: [],
    versions: {},
    cwd: () => current().cwd,
    nextTick: (fn: () => void) => Promise.resolve().then(fn),
    on: () => g.process,
    once: () => g.process,
    off: () => g.process,
    emitWarning: () => {},
  };
  if (g.Buffer) return;
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  class MiniBuffer extends Uint8Array {
    static from(v: unknown): MiniBuffer {
      const b =
        typeof v === "string"
          ? enc.encode(v)
          : new Uint8Array(v as ArrayLike<number>);
      return new MiniBuffer(b.buffer, b.byteOffset, b.byteLength);
    }
    static alloc(n: number): MiniBuffer {
      return new MiniBuffer(n);
    }
    static byteLength(s: string): number {
      return enc.encode(s).length;
    }
    static isBuffer(v: unknown): boolean {
      return v instanceof MiniBuffer;
    }
    static concat(list: Uint8Array[]): MiniBuffer {
      const out = new MiniBuffer(list.reduce((n, b) => n + b.byteLength, 0));
      let at = 0;
      for (const b of list) {
        out.set(b, at);
        at += b.byteLength;
      }
      return out;
    }
    override toString(encoding = "utf8"): string {
      if (encoding === "hex")
        return [...this].map((x) => x.toString(16).padStart(2, "0")).join("");
      return dec.decode(this);
    }
  }
  g.Buffer = MiniBuffer;
}
