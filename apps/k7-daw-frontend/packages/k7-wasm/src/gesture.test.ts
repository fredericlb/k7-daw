// The gestures' TypeScript half: what crosses the ABI is read back as the page expects, and a pointer move's calls stay cheap.
// The arithmetic is held by apps/k7-daw-wasm/gesture's own tests; this holds the marshalling and the cost.

import { describe, expect, it } from "vitest";
import {
  anchorBars,
  barBeat,
  barWindow,
  clampCell,
  clipPlayhead,
  crossfadePair,
  drawSpan,
  frameWindow,
  groupOffset,
  moveOffset,
  planGroupMove,
  rollFit,
  rollPoint,
  selBox,
  snapMode,
  snapPosition,
  snapTo,
  takeCounter,
  tSpan,
  videoDragEdits,
  videoDragMove,
  warpLine,
} from "./gesture.ts";
import {
  formatAt,
  formatClock,
  nearestTick,
  rulerScale,
  rulerTicks,
  secondTicks,
  subdivideGrid,
} from "./gestureTicks.ts";
import { edgeResize, timelineLayout } from "./gestureTimeline.ts";

describe("the typed calls read back as the page expects", () => {
  it("answers one number, a few in the scalars, and a mode by its name", () => {
    expect(snapTo(1.6, 1)).toBe(2);
    expect(snapMode({ shiftKey: false, ctrlKey: true, metaKey: false })).toBe(
      "relative",
    );
    expect(snapPosition(2.3, 1.4, 1, "grid")).toBe(4);
    expect(moveOffset(-100, 10, 1, 2.3, "relative")).toBeCloseTo(-2, 9);
    expect(
      tSpan({ key: 60, time: 4, dur: 1, mode: "move", dBeats: 2, dKeys: 3 }),
    ).toEqual({ key: 63, start: 6, end: 7 });
    expect(drawSpan(4.1, 4.9, 0.25, 0, 0.25)).toEqual({ from: 4, to: 5 });
    expect(rollPoint(40, 30, 8, 20, 16, 12)).toEqual({ beat: 10, key: 125 });
  });

  it("writes an array in, and reads a few numbers back", () => {
    const chord = [
      { key: 60, time: 1, duration: 1 },
      { key: 67, time: 2, duration: 0.5 },
    ];
    expect(groupOffset(chord, -5, 70)).toEqual({ dBeats: -1, dKeys: 60 });
    const fit = rollFit([{ time: 0, duration: 1, key: 60 }], 4, 4, null);
    expect(fit.canvasBeats).toBe(4);
    expect(rollFit([], 0, 4, { from: 8, to: 12, win: 4 }).loBeat).toBe(6);
  });

  it("leaves a number out as NaN and reads an answer with no value as null", () => {
    const act = { from: 0, to: 10 };
    expect(clipPlayhead(30, null, act, 120)).toBeNull();
    expect(clipPlayhead(0, 0, act, 120)).toBe(0);
    expect(nearestTick([0, 10, 20], 12, 1)).toBeNull();
    expect(nearestTick([0, 10, 20], 12)).toBe(10);
    expect(frameWindow({ from: 0, to: 60 }, { from: 20, to: 30 }, 2)).toEqual({
      bars: 7,
      start: 9,
    });
  });

  it("carries a cell, a window, a take and a picture drag across", () => {
    expect(clampCell({ row: 9, col: 9 }, { rows: 4, cols: 8 })).toEqual({
      row: 3,
      col: 7,
    });
    expect(
      selBox({ anchor: { row: 3, col: 5 }, head: { row: 1, col: 2 } }),
    ).toEqual({ rowLo: 1, rowHi: 3, colLo: 2, colHi: 5 });
    const w = barWindow({ from: 0, to: 60 }, 12, 120, 4, 10);
    expect(w.view).toEqual({ start: 20, end: 44 });
    expect(anchorBars(w.view, 0.5, 6, w.barSec)).toBeCloseTo(13, 9);
    expect(takeCounter(56, 58, 4)).toEqual({ phase: "count-in", beats: 2 });
    expect(barBeat(9, 4)).toEqual({ bar: 3, beat: 2 });
    const shot = {
      mode: "in" as const,
      origStart: 4,
      origLength: 6,
      origFrom: 2,
      start: 6,
      length: 4,
    };
    expect(videoDragEdits({ ...shot, index: 2, activation: false })).toEqual([
      { index: 2, field: "start", value: 6 },
      { index: 2, field: "from", value: 4 },
      { index: 2, field: "length", value: 4 },
    ]);
    const snap = { mode: "free" as const, unit: 1, bps: 1, ticks: [] };
    expect(videoDragMove(shot, 200, 100, snap).start).toBe(6);
  });

  it("answers the lists and the texts a ruler draws", () => {
    expect(secondTicks(30).slice(0, 2)).toEqual([1 / 30, 2 / 30]);
    expect(subdivideGrid([0, 1, 2], 80)).toEqual([0.5, 1.5]);
    expect(formatClock(90.5, 0.5)).toBe("1:30.5");
    expect(formatAt(3723)).toBe("1:02:03");
    expect(rulerScale("bars", 10, 4, 2, 64).unit).toBe(4);
    expect(edgeResize("end", 4, 8, 14, 10, 1, "grid")).toEqual({
      start: 4,
      end: 9,
    });
    const l = timelineLayout(null, 20, 1, 1000, 200, 20, 120);
    expect([l.beatHi, l.ppb, l.laneW, l.bps]).toEqual([19, 40, 800, 2]);
  });
});

describe("the calls that answer a structure", () => {
  it("plans a selection drag and refuses a note in the way", () => {
    const chord = [{ key: 60, time: 1, duration: 1 }];
    expect(planGroupMove(chord, [], 1, 2)).toEqual({
      moves: [[60, 1, 62, 2]],
      dBeats: 1,
      dKeys: 2,
    });
    expect(
      planGroupMove(chord, [{ key: 62, time: 2, duration: 1 }], 1, 2),
    ).toEqual({ refused: "occupied" });
  });

  it("words nothing: a line, and the id of its sentence", () => {
    expect(
      warpLine("break", 4, 2, { inPoint: 2, stretch: 0.5, zeroBeat: 0 }, 120),
    ).toBe("clip warp break 10.0000 4.000");
    const clips = [
      { id: "a", start: 0, length: 8, type: "audio" },
      { id: "b", start: 8, length: 8, type: "audio" },
    ];
    expect(crossfadePair(clips, clips[0] as (typeof clips)[0], 7)).toEqual({
      other: 1,
      after: true,
    });
  });

  it("builds the ticks a ruler draws, each keyed from its origin", () => {
    const ticks = rulerTicks("bars", 0, 64, 1, 4, 2, 1, { lo: 10, hi: 20 });
    expect(ticks).toHaveLength(11);
    expect(ticks[0]).toMatchObject({ beat: 10, key: "b:10", index: 10 });
  });
});

describe("what a pointer move costs", () => {
  it("is a few microseconds for the calls one move makes", () => {
    const ev = { shiftKey: false, ctrlKey: false, metaKey: false };
    const run = (n: number) => {
      let sink = 0;
      for (let i = 0; i < n; i++)
        sink += moveOffset(i * 0.7, 24, 0.25, 4, snapMode(ev));
      return sink;
    };
    run(5000);
    const n = 100_000;
    const t0 = performance.now();
    run(n);
    const perMove = ((performance.now() - t0) * 1000) / n;
    // Two calls a move, tens of nanoseconds each on an idle machine; the ceiling is a hundred times that, for a loaded one.
    expect(perMove).toBeLessThan(10);
  });
});
