// The hold-to-drag state machine: a finger that stays arms the drag, one that moves first does nothing, a hold released in place asks for the menu.

import { describe, expect, it } from "vitest";
import {
  HOLD_MS,
  HOLD_SLOP_PX,
  HoldFollow,
  HoldGesture,
  type Timers,
} from "./hold";

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

  it("a move past the slop before the hold does nothing: nothing arms", () => {
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

describe("HoldGesture and a second finger", () => {
  it("a second finger before the hold cancels it: no arm, no tap", () => {
    const { g, log, advance } = rig();
    advance(100);
    g.otherDown();
    advance(HOLD_MS);
    g.up(pt(10, 10));
    expect(log).toEqual([]);
    expect(g.over).toBe(true);
  });

  it("a second finger after the hold leaves the drag armed", () => {
    const { g, log, advance } = rig();
    advance(HOLD_MS);
    g.otherDown();
    expect(g.armed).toBe(true);
    expect(log).toEqual(["armed-hook", "arm"]);
  });
});

function followRig() {
  let clock = 0;
  let pending: { fn: () => void; at: number } | null = null;
  const log: string[] = [];
  const timers: Timers = {
    set: (fn, ms) => {
      pending = { fn, at: clock + ms };
      return pending;
    },
    clear: (h) => {
      if (h === pending) pending = null;
    },
    now: () => clock,
  };
  const f = new HoldFollow(
    { pointerId: 1, clientX: 10, clientY: 10 },
    {
      move: (ev) => log.push(`move ${ev.clientX}`),
      end: (ev) => log.push(`end ${ev.clientX}`),
      cancel: () => log.push("cancel"),
    },
    timers,
    (x, y) => log.push(`menu ${x},${y}`),
    () => log.push("armed"),
  );
  const advance = (ms: number) => {
    clock += ms;
    const p = pending;
    if (p && p.at <= clock) {
      pending = null;
      p.fn();
    }
  };
  return { f, log, advance };
}

const ev = (x: number, y = 10, pointerId = 1) =>
  ({ pointerId, clientX: x, clientY: y }) as PointerEvent;

describe("HoldFollow", () => {
  it("a finger that moves first cancels the drag, and nothing after it counts", () => {
    const { f, log, advance } = followRig();
    f.move(ev(10 + HOLD_SLOP_PX + 1));
    advance(HOLD_MS);
    f.move(ev(80));
    f.end(ev(80));
    expect(log).toEqual(["cancel"]);
  });

  it("moves inside the slop before the hold reach no handler", () => {
    const { f, log } = followRig();
    f.move(ev(12));
    expect(log).toEqual([]);
  });

  it("a tap ends the drag at once, as a click", () => {
    const { f, log, advance } = followRig();
    advance(100);
    f.end(ev(10));
    expect(log).toEqual(["end 10"]);
  });

  it("held then dragged, the moves pass and the release commits", () => {
    const { f, log, advance } = followRig();
    advance(HOLD_MS);
    f.move(ev(40));
    f.move(ev(60));
    f.end(ev(60));
    expect(log).toEqual(["armed", "move 40", "move 60", "end 60"]);
  });

  it("held and released in place, the drag is cancelled and the menu asked for", () => {
    const { f, log, advance } = followRig();
    advance(HOLD_MS);
    f.end(ev(11, 12));
    expect(log).toEqual(["armed", "cancel", "menu 11,12"]);
  });

  it("a second finger during the wait cancels the drag once", () => {
    const { f, log, advance } = followRig();
    f.otherDown();
    advance(HOLD_MS);
    f.end(ev(10));
    f.cancel();
    expect(log).toEqual(["cancel"]);
    expect(f.over).toBe(true);
  });
});
