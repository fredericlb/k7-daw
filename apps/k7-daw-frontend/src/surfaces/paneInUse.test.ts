import { describe, expect, it } from "vitest";
import {
  inUseBy,
  type MainSurfaceIds,
  nextFree,
  sameScreens,
} from "./paneInUse";

const ids: MainSurfaceIds = {
  timeline: "arr",
  turntables: "desk",
  viewport: "frame",
};

describe("inUseBy", () => {
  it("maps a remote's surface to the pane or window it is on this device", () => {
    const screens = [
      { surface: "desk", device: "iPad", since: 1 },
      { surface: "frame", device: "Pixel", since: 2 },
      { surface: "arr", device: "Mac", since: 3 },
    ];
    expect(inUseBy(screens, ids)).toEqual({
      turntables: "iPad",
      viewport: "Pixel",
      timeline: "Mac",
    });
  });

  it("never blocks a surface with no pane here, mastering among them", () => {
    expect(
      inUseBy([{ surface: "mix", device: "iPad", since: 1 }], ids),
    ).toEqual({});
    expect(
      inUseBy([{ surface: "desk", device: "iPad", since: 1 }], {
        ...ids,
        turntables: null,
      }),
    ).toEqual({});
  });

  it("names the device that came first when two remotes show one surface", () => {
    const screens = [
      { surface: "desk", device: "late", since: 9 },
      { surface: "desk", device: "early", since: 2 },
    ];
    expect(inUseBy(screens, ids).turntables).toBe("early");
  });
});

describe("nextFree", () => {
  const order = ["timeline", "turntables", "chains", "editor"];

  it("steps to the next pane that is not blocked, in the switcher's order", () => {
    expect(nextFree(order, "turntables", new Set(["turntables"]))).toBe(
      "chains",
    );
    expect(
      nextFree(order, "timeline", new Set(["timeline", "turntables"])),
    ).toBe("chains");
  });

  it("wraps, and answers null when every other pane is blocked", () => {
    expect(nextFree(order, "editor", new Set(["editor", "timeline"]))).toBe(
      "turntables",
    );
    expect(nextFree(["a", "b"], "a", new Set(["a", "b"]))).toBeNull();
  });
});

describe("sameScreens", () => {
  it("compares by content", () => {
    const a = [{ surface: "desk", device: "iPad", since: 1 }];
    expect(sameScreens(a, [{ ...a[0] }])).toBe(true);
    expect(sameScreens(a, [])).toBe(false);
  });
});
