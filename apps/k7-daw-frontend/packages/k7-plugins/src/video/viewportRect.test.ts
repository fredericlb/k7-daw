// The frame's picking and its eight grips. Both are wrong silently: a click could select the
// layer BEHIND the one you can see, and a resize could slide the edge it was supposed to pin,
// which is a move wearing a resize's clothes.

import { describe, expect, it } from "vitest";
import {
  dragRect,
  gripAt,
  hitLayer,
  layerBelow,
  layersAt,
  type PickLayer,
  type Rect,
  sameRect,
} from "./viewportRect";

const r = (x: number, y: number, w: number, h: number): Rect => ({
  x,
  y,
  w,
  h,
});
const layer = (id: string, rect: Rect, hidden = false): PickLayer => ({
  id,
  rect,
  hidden,
});

describe("hitLayer", () => {
  // `layers` is bottom-first, so the LAST one is the one you can see.
  const stack = [
    layer("main", r(0, 0, 1280, 720)),
    layer("pip", r(960, 540, 320, 180)),
  ];

  it("picks the front layer where two overlap", () => {
    expect(hitLayer(stack, 1000, 600)).toBe("pip");
  });

  it("picks the one behind where the front one is not", () => {
    expect(hitLayer(stack, 100, 100)).toBe("main");
  });

  it("answers null outside every layer", () => {
    expect(hitLayer(stack, 5000, 5000)).toBeNull();
  });

  it("skips a hidden layer — `mute` means it is not on screen to click", () => {
    expect(
      hitLayer([layer("pip", r(0, 0, 100, 100), true)], 50, 50),
    ).toBeNull();
  });
});

// Two full-frame lanes are the ordinary case the moment anyone adds one, and then the click can
// only ever reach the front. Alt cycles, which is the only way down to the one behind.
describe("layersAt / layerBelow", () => {
  const stacked = [
    layer("main", r(0, 0, 1280, 720)),
    layer("over", r(0, 0, 1280, 720)),
  ];

  it("lists what is under the point, front first", () => {
    expect(layersAt(stacked, 100, 100)).toEqual(["over", "main"]);
  });

  it("steps down from the selection, and wraps back to the front", () => {
    expect(layerBelow(stacked, 100, 100, "over")).toBe("main");
    expect(layerBelow(stacked, 100, 100, "main")).toBe("over");
  });

  it("starts at the front when nothing is selected", () => {
    expect(layerBelow(stacked, 100, 100, null)).toBe("over");
  });

  it("answers null where there is nothing at all", () => {
    expect(layerBelow(stacked, 5000, 5000, "over")).toBeNull();
  });
});

describe("gripAt", () => {
  const box = r(100, 100, 200, 200);

  it("names each edge and corner", () => {
    expect(gripAt(box, 100, 100)).toBe("nw");
    expect(gripAt(box, 300, 300)).toBe("se");
    expect(gripAt(box, 200, 100)).toBe("n");
    expect(gripAt(box, 300, 200)).toBe("e");
  });

  it("prefers a corner to an edge, since on a small layer the bands overlap", () => {
    expect(gripAt(r(0, 0, 16, 16), 0, 0)).toBe("nw");
  });

  it("calls the middle the body", () => {
    expect(gripAt(box, 200, 200)).toBe("body");
  });

  it("misses outside the rect and its tolerance", () => {
    expect(gripAt(box, 50, 200)).toBeNull();
  });
});

describe("dragRect", () => {
  // The rule is Rust's, tested in `eyes/tests/rules.rs`; this holds the wrapper to the page's rect shape.
  const box = r(100, 100, 200, 200);

  it("answers the rect a grip drag lands on", () => {
    expect(dragRect(box, "body", 10, -20)).toEqual(r(110, 80, 200, 200));
    expect(dragRect(box, "w", 50, 0)).toEqual(r(150, 100, 150, 200));
    expect(dragRect(box, "se", 40, 60)).toEqual(r(100, 100, 240, 260));
  });

  it("a drag that came back where it started writes nothing", () => {
    expect(sameRect(box, dragRect(box, "body", 0, 0))).toBe(true);
  });
});
