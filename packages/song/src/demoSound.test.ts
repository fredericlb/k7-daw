// The bundled demos ship to every account's Examples: each of their sound files must have sound in it.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const REPO = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const DEMOS = join(REPO, "demos");

function wavs(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return wavs(path);
    return name.endsWith(".wav") ? [path] : [];
  });
}

// Peak of a 16-bit PCM wav, 0..1; the demos' files are all that format.
function peak(file: string): number {
  const b = readFileSync(file);
  expect(b.toString("latin1", 0, 4)).toBe("RIFF");
  let at = 12;
  let bits = 0;
  while (at + 8 <= b.length) {
    const id = b.toString("latin1", at, at + 4);
    const size = b.readUInt32LE(at + 4);
    if (id === "fmt ") bits = b.readUInt16LE(at + 8 + 14);
    if (id === "data") {
      expect(bits).toBe(16);
      let max = 0;
      const end = Math.min(b.length, at + 8 + size);
      for (let i = at + 8; i + 1 < end; i += 2)
        max = Math.max(max, Math.abs(b.readInt16LE(i)));
      return max / 32768;
    }
    at += 8 + size + (size % 2);
  }
  throw new Error(`${file}: no data chunk`);
}

describe("the bundled demos' sound files", () => {
  it.each(wavs(DEMOS).map((f) => relative(DEMOS, f)))(
    "%s is audible",
    (rel) => {
      expect(peak(join(DEMOS, rel))).toBeGreaterThan(0.05);
    },
  );
});
