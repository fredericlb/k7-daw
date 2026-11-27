import { describe, expect, it } from "vitest";
import { patchWebmDuration } from "./webmDuration";

// The head's bytes are Rust's (waveform's webm_duration tests); this is the Blob it is spliced into.
const el = (id: number[], data: number[], unknownSize = false) => [
  ...id,
  ...(unknownSize
    ? [0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    : [0x80 | data.length]),
  ...data,
];

const EBML = el([0x1a, 0x45, 0xdf, 0xa3], el([0x42, 0x82], [0x77, 0x65]));
const SCALE = el([0x2a, 0xd7, 0xb1], [0x0f, 0x42, 0x40]);
const INFO = el([0x15, 0x49, 0xa9, 0x66], [...SCALE, ...el([0x4d, 0x80], [])]);
const CLUSTER = [0x1f, 0x43, 0xb6, 0x75, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff];
CLUSTER.push(0xff, 0xff, 1, 2, 3, 4);

describe("patchWebmDuration", () => {
  it("keeps the tail as the original bytes", async () => {
    const seg = el([0x18, 0x53, 0x80, 0x67], [...INFO, ...CLUSTER], true);
    const file = new Uint8Array([...EBML, ...seg]);
    const blob = new Blob([file], { type: "audio/webm" });
    const fixed = await patchWebmDuration(blob, 2000);
    if (!fixed) throw new Error("not patched");
    expect(fixed.size).toBe(file.length + 11);
    expect(fixed.type).toBe("audio/webm");
    const bytes = new Uint8Array(await fixed.arrayBuffer());
    expect([...bytes.subarray(bytes.length - 4)]).toEqual([1, 2, 3, 4]);
  });

  it("answers null for a layout the patcher does not know", async () => {
    const odd = new Blob([new Uint8Array([1, 2, 3])]);
    expect(await patchWebmDuration(odd, 5)).toBeNull();
  });
});
