// A split is two placements of one clip, and must play exactly as the one did: a tail appended at the end of the file,
// a tail that restarts its source and a fade left on the wrong side of the cut would all be silent. The writers are Rust's.

import { describe, expect, it } from "vitest";
import { parse } from "./k7x/index.ts";
import { editVideo, listVideoPlacements, type VideoOp } from "./text.ts";

/** One op run over a whole file's text: the text and what the writer answered. */
const edit = (text: string, op: VideoOp, at: (string | number)[] = []) => {
  const out = editVideo(text, [op], at);
  return { result: out.results[0], text: out.text };
};

const events = (text: string) =>
  parse(text).events as Record<string, unknown>[];

const LANE = `// the A-roll
extends("@meta/picture-track")
events([
  ref("a").start(2).length(6).from(1).fadeIn(0.5).fadeOut({ over: 1, with: "wipe" }),
  ref("b").start(10).length(0),
])
`;

const split = (text: string, index: number, cut: number) =>
  edit(text, { op: "splitAt", index, cut });

describe("splitAt", () => {
  it("cuts a shot into a head and a tail that play the same frames, the tail right after the head", () => {
    const out = split(LANE, 0, 5);
    expect(out.result).toBe(true);
    expect(events(out.text)).toEqual([
      { ref: "a", start: 2, length: 3, from: 1, fadeIn: 0.5 },
      {
        ref: "a",
        start: 5,
        length: 3,
        from: 4,
        fadeOut: { over: 1, with: "wipe" },
      },
      { ref: "b", start: 10, length: 0 },
    ]);
    expect(out.text).toContain("// the A-roll");
  });

  it("keeps a length-0 tail open, running to the next start as the shot did", () => {
    expect(listVideoPlacements(split(LANE, 1, 12).text).slice(1)).toEqual([
      { ref: "b", start: 10, length: 2, sourceFrom: 0 },
      { ref: "b", start: 12, length: 0, sourceFrom: 2 },
    ]);
  });

  it("splits an activation without inventing a source to trim", () => {
    const out = split("events([start(0).length(4)])\n", 0, 1);
    expect(events(out.text)).toEqual([
      { start: 0, length: 1 },
      { start: 1, length: 3 },
    ]);
  });

  it("cuts the animation exactly, each half playing what the shot did", () => {
    const text = `events([
  ref("a").start(0).length(8).animate({ x: [[0, 0], [8, 800]], opacity: 0.5 }),
])
`;
    const out = events(split(text, 0, 2).text);
    expect(out[0]?.animate).toEqual({
      x: [
        [0, 0],
        [8, 800],
      ],
      opacity: 0.5,
    });
    // The tail starts 2 s in, so its keys are the same keys two seconds earlier.
    expect(out[1]?.animate).toEqual({
      x: [
        [-2, 0],
        [6, 800],
      ],
      opacity: 0.5,
    });
  });

  it("refuses a cut on an edge or outside the shot, leaving the file alone", () => {
    for (const [i, cut] of [
      [0, 2],
      [0, 8],
      [0, 9],
      [5, 3],
    ])
      expect(split(LANE, i, cut)).toEqual({ result: false, text: LANE });
  });
});

describe("shiftAnimationAt", () => {
  it("moves the keys back by a trimmed head, so they stay on the same frames", () => {
    const text = `events([
  ref("a").start(0).length(8).animate({ scale: [[0, 1], { t: 4, v: 2, ease: "out" }], x: 10 }),
])
`;
    const out = edit(text, { op: "shiftAnimationAt", index: 0, delta: 1.5 });
    expect(events(out.text)[0]?.animate).toEqual({
      scale: [[-1.5, 1], { t: 2.5, v: 2, ease: "out" }],
      x: 10,
    });
  });

  it("leaves a shot with no animation untouched", () => {
    expect(edit(LANE, { op: "shiftAnimationAt", index: 0, delta: 1 })).toEqual({
      result: false,
      text: LANE,
    });
  });
});

describe("the lane's other edits", () => {
  const add: VideoOp = {
    op: "addShot",
    ref: "c",
    start: 12,
    length: 2,
    sourceFrom: 0,
  };

  it("appends a placement on a line of its own, keeping the others", () => {
    const out = edit(LANE, add);
    expect(out.result).toBe(true);
    expect(out.text).toContain('ref("b").start(10).length(0),');
    expect(events(out.text)[2]).toEqual({ ref: "c", start: 12, length: 2 });
  });

  it("creates `events` on an empty lane, and refuses one holding something else", () => {
    const fresh = edit("", { ...add, start: 0, length: 1 });
    expect(events(fresh.text)).toEqual([{ ref: "c", start: 0, length: 1 }]);
    const odd = 'events("none")\n';
    expect(edit(odd, add)).toEqual({ result: false, text: odd });
  });

  it("renames a fade's chain, and only a fade's", () => {
    const text =
      'events([ref("a").start(0).fadeIn({ over: 1, chain: "iris" })])\n';
    const out = edit(text, {
      op: "renameFadeChains",
      renames: [["iris", "wipe"]],
    });
    expect(out.result).toBe(true);
    expect(out.text).toContain('chain: "wipe"');
    expect(
      edit(text, { op: "renameFadeChains", renames: [["x", "y"]] }).result,
    ).toBe(false);
  });

  it("writes the lane's rect as `frame`, all four values, rounded to whole pixels", () => {
    const out = edit(LANE, {
      op: "setLaneRect",
      x: 10.4,
      y: 20,
      w: 320,
      h: 180.6,
    });
    expect(parse(out.text).frame).toEqual([10, 20, 320, 181]);
    expect(out.text).toContain("// the A-roll");
  });

  it("reads nothing from a half-typed buffer, and edits nothing in it", () => {
    const broken = "events([ref(";
    expect(listVideoPlacements(broken)).toEqual([]);
    expect(edit(broken, add)).toEqual({ result: false, text: broken });
  });

  it("sets speed, silence and removal by index, the defaults leaving no key", () => {
    const text = 'events([ref("a").start(0).length(2), ref("b").start(2)])\n';
    const sped = edit(text, { op: "setSpeedAt", index: 0, speed: -0.5 });
    expect(listVideoPlacements(sped.text)[0]?.speed).toBe(-0.5);
    expect(
      edit(sped.text, { op: "setSpeedAt", index: 0, speed: 1 }).text,
    ).not.toContain("speed");
    const hushed = edit(text, { op: "setSilentAt", index: 1, silent: true });
    expect(listVideoPlacements(hushed.text)[1]?.silent).toBe(true);
    expect(
      edit(hushed.text, { op: "setSilentAt", index: 1, silent: false }).text,
    ).toBe(text);
    expect(edit(text, { op: "removeAt", index: 7 }).result).toBe(false);
    expect(
      listVideoPlacements(edit(text, { op: "removeAt", index: 0 }).text),
    ).toHaveLength(1);
  });

  it("addresses the lane of a timeline file at its own key path", () => {
    const text = `pictures([\n  { lane: "a", events: [ref("x").start(0).length(2)] },\n  { lane: "b", events: [ref("y").start(1).length(3)] },\n])\n`;
    const at = ["pictures", 1];
    expect(listVideoPlacements(text, at).map((p) => p.ref)).toEqual(["y"]);
    const out = edit(
      text,
      { op: "setFieldAt", index: 0, field: "start", value: 4.00004 },
      at,
    );
    expect(out.result).toBe(true);
    // Rounded to the millisecond, one line changed.
    expect(out.text).toBe(text.replace("start(1)", "start(4)"));
  });
});

describe("a placement listed and edited by index", () => {
  it("is still LISTED without a ref, so an index addresses the same line in the file", () => {
    const text =
      'extends("@meta/picture-track")\nevents([start(0).length(2), ref("a").start(4)])\n';
    expect(listVideoPlacements(text).map((p) => p.ref)).toEqual(["", "a"]);
    // Index 1 is the shot, whatever precedes it: the verbs and the drag both write this way.
    const out = edit(text, {
      op: "setFieldAt",
      index: 1,
      field: "start",
      value: 6,
    });
    expect(out.result).toBe(true);
    expect(out.text).toContain('ref("a").start(6)');
  });

  it("sets a field by REF, the first placement naming it", () => {
    const text = 'events([ref("a").start(0).from(1), ref("a").start(5)])\n';
    const out = edit(text, {
      op: "setFieldByRef",
      ref: "a",
      field: "from",
      value: 2.5,
    });
    expect(out.text).toBe(text.replace("from(1)", "from(2.5)"));
    expect(
      edit(text, { op: "setFieldByRef", ref: "z", field: "from", value: 1 })
        .result,
    ).toBe(false);
  });
});
