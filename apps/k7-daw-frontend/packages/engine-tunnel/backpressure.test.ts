// backpressure.test.ts — a peer that stops reading: replaceable frames are dropped, a reply is
// queued until the hard limit and then the peer is closed. A tunnel's stalled channel is the Rust hub's (tests/).

import { encodeStreamFrame } from "@studio/engine-protocol/stream";
import { describe, expect, it } from "vitest";
import {
  admit,
  HeldFrames,
  isDroppable,
  MAX_HELD_FRAMES,
  sendBounded,
} from "./backpressure.ts";

const LIMITS = { soft: 100, hard: 1000 };
const audio = Buffer.from(
  encodeStreamFrame({
    version: 1,
    channels: 2,
    sampleRate: 48000,
    frames: 4,
    seq: 1,
    codec: 0,
    captureMs: 0,
    samples: new Float32Array(8),
  }),
);
const state = JSON.stringify({ levels: [0], playing: true, type: "state" });
const reply = JSON.stringify({ type: "plugins", plugins: [] });

describe("what may be dropped", () => {
  it("is an audio broadcast frame, a state or a meter feed, never a reply", () => {
    expect(isDroppable(audio, true)).toBe(true);
    expect(isDroppable(state, false)).toBe(true);
    expect(isDroppable(JSON.stringify({ type: "io-levels" }), false)).toBe(
      true,
    );
    expect(isDroppable(reply, false)).toBe(false);
    expect(isDroppable(Buffer.from("not a stream frame"), true)).toBe(false);
    expect(isDroppable("not json", false)).toBe(false);
  });
});

describe("admit", () => {
  it("sends everything below the soft limit", () => {
    expect(admit(99, audio, true, LIMITS)).toBe("send");
    expect(admit(99, reply, false, LIMITS)).toBe("send");
  });

  it("drops a replaceable frame past the soft limit, and still queues a reply", () => {
    expect(admit(500, audio, true, LIMITS)).toBe("drop");
    expect(admit(500, state, false, LIMITS)).toBe("drop");
    expect(admit(500, reply, false, LIMITS)).toBe("send");
  });

  it("gives up on the peer when replies alone pass the hard limit", () => {
    expect(admit(1000, reply, false, LIMITS)).toBe("close");
    expect(admit(1000, state, false, LIMITS)).toBe("drop");
  });

  it("sendBounded calls the overrun instead of sending", () => {
    const sent: unknown[] = [];
    let overrun = 0;
    const peer = { bufferedAmount: 2000, send: (d: unknown) => sent.push(d) };
    sendBounded(peer, reply, false, () => overrun++, LIMITS);
    sendBounded(peer, audio, true, () => overrun++, LIMITS);
    expect(sent).toEqual([]);
    expect(overrun).toBe(1);
  });
});

describe("HeldFrames", () => {
  it("refuses past its frame bound, and gives back what it held in order", () => {
    const h = new HeldFrames<string>();
    for (let i = 0; i < MAX_HELD_FRAMES; i++)
      expect(h.push(`${i}`, false)).toBe(true);
    expect(h.push("one more", false)).toBe(false);
    const out = h.drain();
    expect(out).toHaveLength(MAX_HELD_FRAMES);
    expect(out[0].data).toBe("0");
    expect(h.push("again", false)).toBe(true);
  });
});
