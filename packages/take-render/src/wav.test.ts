import { describe, expect, it } from "vitest";
import { floatFormat, wavHeader, wavLayout } from "./wav";

/** A mono 16-bit `fmt ` body at `rate`. */
function pcmFormat(rate: number): Buffer {
  const f = Buffer.alloc(16);
  f.writeUInt16LE(1, 0);
  f.writeUInt16LE(1, 2);
  f.writeUInt32LE(rate, 4);
  f.writeUInt32LE(rate * 2, 8);
  f.writeUInt16LE(2, 12);
  f.writeUInt16LE(16, 14);
  return f;
}

/** A WAV of `frames` mono 16-bit frames, with a LIST chunk before its data, as many recorders write. */
function wav(frames: number, rate: number, declared = frames * 2) {
  const data = Buffer.alloc(frames * 2);
  for (let i = 0; i < frames; i++) data.writeInt16LE(i, i * 2);
  const list = Buffer.from("LIST\x04\x00\x00\x00INFO", "latin1");
  const head = wavHeader(pcmFormat(rate), declared);
  const fmtEnd = head.length - 8;
  return Buffer.concat([
    head.subarray(0, fmtEnd),
    list,
    head.subarray(fmtEnd),
    data,
  ]);
}

describe("a WAV's layout", () => {
  it("finds the samples past other chunks", () => {
    const b = wav(10, 1000);
    const w = wavLayout(b, b.length);
    expect(w).toMatchObject({
      blockAlign: 2,
      sampleRate: 1000,
      dataBytes: 20,
      declaredBytes: 20,
    });
    expect(b.readInt16LE((w?.dataOffset ?? 0) + 4)).toBe(2);
  });

  it("reads an unclosed file's data as what is there, and keeps the size it declared", () => {
    const b = wav(10, 1000, 0);
    expect(wavLayout(b, b.length)).toMatchObject({
      dataBytes: 20,
      declaredBytes: 0,
    });
  });

  it("clamps a size past the end to whole frames on disk", () => {
    const b = Buffer.concat([wav(10, 1000, 4000), Buffer.alloc(1)]);
    expect(wavLayout(b, b.length)).toMatchObject({
      dataBytes: 20,
      declaredBytes: 4000,
    });
  });

  it("is none for a WebM or a compressed WAV", () => {
    const webm = Buffer.from([0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0, 0, 0, 0, 0]);
    expect(wavLayout(webm, webm.length)).toBeNull();
    const b = wav(10, 1000);
    b.writeUInt16LE(85, 20);
    expect(wavLayout(b, b.length)).toBeNull();
  });
});

describe("a float WAV's header", () => {
  it("is the canonical 44 bytes, and reads back as what it describes", () => {
    const h = wavHeader(floatFormat(2, 48000), 800);
    expect(h.length).toBe(44);
    expect(h.readUInt32LE(4)).toBe(36 + 800);
    const w = wavLayout(Buffer.concat([h, Buffer.alloc(800)]), 844);
    expect(w).toMatchObject({
      blockAlign: 8,
      sampleRate: 48000,
      dataBytes: 800,
    });
  });
});
