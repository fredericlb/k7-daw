import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { floatFormat, wavHeader } from "@studio/audio-analysis/wav";
import { afterAll, describe, expect, it } from "vitest";
import { wavSeconds } from "./wavLength";

const dir = mkdtempSync(join(tmpdir(), "k7-wavlength-"));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

/** A stereo float wav at 1000 Hz declaring `declared` bytes and holding `held`. */
function write(name: string, declared: number, held: number): string {
  const p = join(dir, name);
  writeFileSync(
    p,
    Buffer.concat([
      wavHeader(floatFormat(2, 1000), declared),
      Buffer.alloc(held),
    ]),
  );
  return p;
}

describe("a wav's length", () => {
  it("is its data over its frame rate", () => {
    expect(wavSeconds(write("full.wav", 8000, 8000))).toBe(1);
  });

  it("clamps a size past the end to what the disk holds", () => {
    expect(wavSeconds(write("cut.wav", 80000, 4000))).toBe(0.5);
  });

  it("is 0 for a file whose header says 0, whatever follows", () => {
    expect(wavSeconds(write("open.wav", 0, 4000))).toBe(0);
  });

  it("is null for what is no wav", () => {
    const p = join(dir, "x.webm");
    writeFileSync(p, Buffer.alloc(64, 1));
    expect(wavSeconds(p)).toBeNull();
    expect(wavSeconds(join(dir, "missing.wav"))).toBeNull();
  });
});
