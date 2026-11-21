// The hold-to-drag state machine: a finger that stays arms the drag, one that moves first scrolls, a hold released in place asks for the menu.

import { describe, expect, it } from "vitest";
import { HOLD_MS, HOLD_SLOP_PX, HoldGesture } from "./holdDrag";

function rig() {
  let clock = 0;
  let pending: { fn: () => void; at: number } | null = null;
  const log: string[] = [];
  const g = new HoldGesture(
    { pointerId: 1, clientX: 10, clientY: 10 },
    {
      arm: () => log.push("arm"),
      tap: () => log.push("tap"),
      release: (x, y) => log.push(`release ${x},${y}`),
    },
    {
      set: (fn, ms) => {
        pending = { fn, at: clock + ms };
        return pending;
      },
      clear: (h) => {
        if (h === pending) pending = null;
      },
      now: () => clock,
    },
    () => log.push("armed-hook"),
  );
  const advance = (ms: number) => {
    clock += ms;
    const p = pending;
    if (p && p.at <= clock) {
      pending = null;
      p.fn();
    }
  };
  return { g, log, advance };
}

const pt = (x: number, y: number, pointerId = 1) => ({
  pointerId,
  clientX: x,
  clientY: y,
});

describe("HoldGesture", () => {
  it("arms after the hold, once", () => {
    const { g, log, advance } = rig();
    advance(HOLD_MS - 1);
    expect(log).toEqual([]);
    advance(1);
    advance(HOLD_MS);
    expect(log).toEqual(["armed-hook", "arm"]);
    expect(g.armed).toBe(true);
  });

  it("a move past the slop before the hold hands the touch to the scroll: nothing arms", () => {
    const { g, log, advance } = rig();
    g.move(pt(10, 10 + HOLD_SLOP_PX + 1));
    advance(HOLD_MS * 2);
    expect(log).toEqual([]);
    expect(g.over).toBe(true);
  });

  it("a tremor inside the slop does not cancel the hold", () => {
    const { g, log, advance } = rig();
    g.move(pt(10 + HOLD_SLOP_PX, 10));
    advance(HOLD_MS);
    expect(log).toEqual(["armed-hook", "arm"]);
  });

  it("a release before the hold is a tap", () => {
    const { g, log, advance } = rig();
    advance(100);
    g.up(pt(10, 10));
    advance(HOLD_MS);
    expect(log).toEqual(["tap"]);
  });

  it("an armed hold released in place asks for the menu where it is", () => {
    const { g, log, advance } = rig();
    advance(HOLD_MS);
    g.up(pt(12, 11));
    expect(log).toEqual(["armed-hook", "arm", "release 12,11"]);
  });

  it("an armed hold that dragged opens no menu on release", () => {
    const { g, log, advance } = rig();
    advance(HOLD_MS);
    g.move(pt(60, 10));
    g.up(pt(60, 10));
    expect(log).toEqual(["armed-hook", "arm"]);
  });

  it("another finger's events are ignored, and a cancel writes nothing", () => {
    const { g, log, advance } = rig();
    g.move(pt(200, 200, 2));
    g.up(pt(10, 10, 2));
    advance(HOLD_MS);
    g.cancel();
    expect(log).toEqual(["armed-hook", "arm"]);
    expect(g.over).toBe(true);
  });
});
