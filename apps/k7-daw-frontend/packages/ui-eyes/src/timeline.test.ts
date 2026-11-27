// The lookup half of the transition rule, end to end: Rust's schedule (the module) read by the page's `layerFrameAt`.
// The rule's own cases are Rust tests (`apps/k7-daw-wasm/eyes/tests/schedule.rs`); this holds what only the page does.

import { describe, expect, it } from "vitest";
import type { SceneClip, SceneFade, VideoLayer } from "./scene";
import { layerOf } from "./sceneFixture";
import {
  activeAt,
  layerFrameAt,
  shapesVisibleAt,
  slotActive,
  slotEffects,
} from "./timeline";

const clip = (
  start: number,
  length: number,
  id = `c${start}`,
  extra: Partial<SceneClip> = {},
): SceneClip => ({
  id,
  src: `${id}.mp4`,
  start,
  length,
  sourceFrom: 0,
  ...extra,
});

const layer = (clips: SceneClip[]): VideoLayer =>
  layerOf({ id: "cam", x: 0, y: 0, w: 1920, h: 1080, clips });

const dissolve = (over: number): SceneFade => ({ over, with: "dissolve" });

describe("activeAt picks the LAST active, not the first", () => {
  // The first would make a completed dissolve snap back to the outgoing shot for its last frames.
  it("returns the incoming shot inside an overlap", () => {
    const l = layer([clip(0, 5), clip(4, 5)]);
    expect(activeAt(l, 4.5)?.clip.id).toBe("c4");
    expect(activeAt(l, 4.99)?.clip.id).toBe("c4");
  });

  it("is null on an empty lane and in a gap", () => {
    expect(activeAt(layer([]), 0)).toBeNull();
    expect(activeAt(layer([clip(0, 1), clip(4, 1)]), 2)).toBeNull();
  });
});

describe("what the page works out from a span", () => {
  it("maps into the source by sourceFrom + elapsed", () => {
    const l = layer([{ ...clip(4, 4), sourceFrom: 2.5 }]);
    expect(activeAt(l, 5)?.sourceTime).toBe(3.5);
  });

  it("…times the speed, signed: faster, frozen, or backwards from the in-point", () => {
    const at = (speed: number) =>
      activeAt(layer([{ ...clip(4, 4), sourceFrom: 2.5, speed }]), 5)
        ?.sourceTime;
    expect(at(2)).toBe(4.5);
    expect(at(0)).toBe(2.5);
    expect(at(-1)).toBe(1.5);
  });

  it("gives a shot its end, and an open-ended one Infinity", () => {
    const l = layer([clip(0, 4), clip(6, 0)]);
    expect(activeAt(l, 1)?.end).toBe(4);
    expect(activeAt(l, 7)?.end).toBe(Number.POSITIVE_INFINITY);
  });

  it("keeps a clip's index, since a clip's id repeats", () => {
    const l = layer([clip(0, 2, "x"), clip(2, 2, "x")]);
    expect(activeAt(l, 3)?.index).toBe(1);
  });
});

describe("layerFrameAt", () => {
  it("is a plain cut when nothing fades", () => {
    const f = layerFrameAt(layer([clip(0, 4)]), 1);
    expect(f.blend).toBeNull();
    expect(f.to?.clip.id).toBe("c0");
    expect(f.from).toBeNull();
  });

  it("blends the incoming shot OVER the outgoing one, 0 to 1", () => {
    const l = layer([clip(0, 5), clip(4, 5, "c4", { fadeIn: dissolve(1) })]);
    expect(layerFrameAt(l, 4).blend?.progress).toBe(0);
    expect(layerFrameAt(l, 4.5).blend?.progress).toBeCloseTo(0.5);
    const mid = layerFrameAt(l, 4.5);
    expect(mid.from?.clip.id).toBe("c0");
    expect(mid.to?.clip.id).toBe("c4");
  });

  it("draws the incoming shot alone once the window is over — no snap back", () => {
    const l = layer([clip(0, 6), clip(4, 5, "c4", { fadeIn: dissolve(1) })]);
    expect(layerFrameAt(l, 4.9).blend).not.toBeNull();
    const f = layerFrameAt(l, 5.5);
    expect(f.blend).toBeNull();
    expect(f.to?.clip.id).toBe("c4");
  });

  it("fades out toward nothing, over the shot's own end", () => {
    const l = layer([clip(0, 4, "c0", { fadeOut: dissolve(2) })]);
    expect(layerFrameAt(l, 1).blend).toBeNull();
    const f = layerFrameAt(l, 3);
    expect(f.from?.clip.id).toBe("c0");
    expect(f.to).toBeNull();
    expect(f.blend?.progress).toBeCloseTo(0.5);
  });

  it("carries the NAMED kind through, so the renderer knows which program to run", () => {
    const l = layer([clip(0, 4, "c0", { fadeIn: { over: 1, with: "wipe" } })]);
    expect(layerFrameAt(l, 0.5).blend?.name).toBe("wipe");
  });

  it("shows nothing on an empty lane", () => {
    expect(layerFrameAt(layer([]), 0)).toEqual({
      from: null,
      to: null,
      blend: null,
    });
  });
});

describe("a fade's STRETCH", () => {
  const over = (chain: string, s = 1): SceneFade => ({
    over: s,
    with: "over",
    chain,
  });

  it("runs the incoming shot through it, its presence rising 0 to 1", () => {
    const l = layer([clip(0, 4), clip(3, 4, "c3", { fadeIn: over("iris") })]);
    expect(layerFrameAt(l, 3.25).blend).toEqual({
      name: "over",
      progress: 0.25,
      stretch: { to: { chain: "iris", presence: 0.25 } },
    });
  });

  it("runs the leaving shot through ITS fade-out's, presence falling 1 to 0", () => {
    const l = layer([clip(0, 4, "c0", { fadeOut: over("zoom", 2) })]);
    expect(layerFrameAt(l, 3.5).blend?.stretch).toEqual({
      from: { chain: "zoom", presence: 0.25 },
    });
  });

  it("carries no stretch at all on a plain dissolve — the ordinary case stays as it was", () => {
    const l = layer([clip(0, 4), clip(3, 4, "c3", { fadeIn: dissolve(1) })]);
    expect(layerFrameAt(l, 3.5).blend).toEqual({
      name: "dissolve",
      progress: 0.5,
    });
  });
});

describe("a layer of SHAPES, whose clips are ACTIVATION spans", () => {
  // An activation carries no picture, so it has no src and no in-point — only when the lane is on.
  const on = (start: number, length: number): SceneClip => ({
    id: "",
    src: "",
    start,
    length,
    sourceFrom: 0,
  });

  it("is on throughout when it states no span at all", () => {
    const l = { ...layer([]), shapes: [] };
    for (const t of [0, 5, 1e6]) expect(shapesVisibleAt(l, t)).toBe(true);
  });

  it("is on only inside the spans it states, by the interval rule the clips use", () => {
    const l = layer([on(2, 2), on(6, 2)]);
    expect([0, 1.9, 4, 5.9, 8, 99].map((t) => shapesVisibleAt(l, t))).toEqual([
      false,
      false,
      false,
      false,
      false,
      false,
    ]);
    expect([2, 3.9, 6, 7.9].map((t) => shapesVisibleAt(l, t))).toEqual([
      true,
      true,
      true,
      true,
    ]);
  });
});

describe("slotActive", () => {
  it("puts a cut's picture in slot 0 and a blend's two sides in 0 and 1", () => {
    const l = layer([
      clip(0, 4, "a"),
      clip(3, 4, "b", { fadeIn: dissolve(1) }),
    ]);
    const cut = layerFrameAt(l, 1);
    expect(slotActive(cut, 0)?.clip.id).toBe("a");
    expect(slotActive(cut, 1)).toBeNull();
    const blend = layerFrameAt(l, 3.5);
    expect(slotActive(blend, 0)?.clip.id).toBe("a");
    expect(slotActive(blend, 1)?.clip.id).toBe("b");
  });

  it("leaves slot 0 empty for a fade from nothing", () => {
    const frame = layerFrameAt(
      layer([clip(2, 4, "b", { fadeIn: dissolve(1) })]),
      2.5,
    );
    expect(slotActive(frame, 0)).toBeNull();
    expect(slotActive(frame, 1)?.clip.id).toBe("b");
  });

  it("reads each slot's effect", () => {
    const l = layer([clip(0, 4, "a", { effect: "iris" })]);
    expect(slotEffects(layerFrameAt(l, 1))).toEqual(["iris", null]);
  });
});
