// The takes view's arithmetic: an event found from any of its pieces, which spans each row is heard
// over, and the two lines it sends — a pick and an arm.

import { describe, expect, it } from "vitest";
import {
  armLine,
  compFadeLine,
  compLine,
  compSeams,
  envelopePath,
  eventIdOf,
  heardSpans,
  pieceEdgeLines,
  snapSpan,
  TAKE_COLORS,
  takeColor,
  takeNumber,
  takesEventOf,
} from "./takes";
import type { WebProject } from "./types";

// A riff at bar 2 (beat 4), its take t2 played over beats 6..8 — two pieces on the lane.
const project = {
  timeSigNumerator: 4,
  timeSigDenominator: 4,
  tracks: [
    {
      id: "keys",
      clips: [
        {
          id: "riff@4",
          type: "midi",
          name: "riff",
          start: 4,
          length: 2,
          comp: { of: "riff@4", take: "riff" },
          event: {
            takes: [{ ref: "t2", start: "2:3" }],
            comp: [{ take: "t2", from: "2:3", to: "3:1" }],
            crossfades: [{ at: "2:3", length: "0:0:1" }],
          },
          notes: [],
        },
        {
          id: "t2@6",
          type: "midi",
          name: "t2",
          start: 6,
          length: 2,
          comp: { of: "riff@4", take: "t2" },
          notes: [],
        },
        {
          id: "other@16",
          type: "midi",
          name: "other",
          start: 16,
          length: 4,
          notes: [],
        },
      ],
    },
  ],
} as unknown as WebProject;

// The rules are Rust's (song reads_ports_takes.rs); what is tested here is the page's own clips coming back and the calls reaching the module.
describe("the takes view over the module", () => {
  it("hands back the page's own clips as the event's pieces, found from any of them", () => {
    const piece = project.tracks[0].clips[1];
    expect(eventIdOf(piece)).toBe("riff@4");
    const ev = takesEventOf(project, "keys", eventIdOf(piece));
    expect(ev?.pieces).toEqual([
      project.tracks[0].clips[0],
      project.tracks[0].clips[1],
    ]);
    expect(takesEventOf(project, "keys", "nope@1")).toBeNull();
    expect(takesEventOf(project, "gone", "riff@4")).toBeNull();
  });

  it("sends the lines through the event's own address", () => {
    const ev = takesEventOf(project, "keys", "riff@4");
    if (!ev) throw new Error("no event");
    expect(heardSpans(ev, ev.rows[1], 10)).toEqual([{ from: 6, to: 8 }]);
    expect(compLine(ev, "t2", { from: 6, to: 8 }, 4, 4)).toBe(
      "event comp keys 2:1 t2 2:3 3:1",
    );
    expect(armLine(ev, 6, null, 4, 4)).toBe("event take arm keys 2:1 2:3");
    expect(compSeams(ev)).toEqual([6]);
    expect(compFadeLine(ev, 6, 0.5, 4, 4)).toBe(
      "event comp fade keys 2:1 2:3 0:0:1",
    );
    expect(pieceEdgeLines(ev, ev.pieces[1], 5, 8, 4, 4)).toEqual([
      "event comp keys 2:1 t2 2:2 3:1",
    ]);
  });
});

describe("the page's own take helpers", () => {
  it("snaps a drag to the grid, and takes a click for no span", () => {
    expect(snapSpan(6.9, 5.1, 1)).toEqual({ from: 5, to: 7 });
    expect(snapSpan(6.1, 6.2, 1)).toBeNull();
  });

  it("colours each take as its row", () => {
    const ev = takesEventOf(project, "keys", "riff@4");
    if (!ev) throw new Error("no event");
    expect(takeColor(ev, "riff")).toBe(TAKE_COLORS[0]);
    expect(takeColor(ev, "t2")).toBe(TAKE_COLORS[1]);
  });
});

describe("envelopePath — a take row's envelope", () => {
  it("draws one mirrored bar per bucket, the loudest at full height", () => {
    expect(envelopePath([0.5, 1], 2, 10)).toBe(
      "M10 0.275h1v0.45h-1ZM11 0.04999999999999999h1v0.9h-1Z",
    );
  });

  // A 24 MB path, linear but about a second alone: a loaded CI host needs more than vitest's 5 s.
  it("survives an envelope far longer than an argument list", () => {
    const rms = Array.from({ length: 300_000 }, (_, i) => i % 7);
    expect(envelopePath(rms, 1, 0).length).toBeGreaterThan(0);
  }, 30_000);

  it("draws nothing for no samples", () => {
    expect(envelopePath([], 4, 0)).toBe("");
  });
});

describe("takeNumber", () => {
  it("counts takes from 1, past the event's own row", () => {
    const rows = [
      { ref: "own", start: 0, own: true },
      { ref: "a", start: 0, own: false },
      { ref: "b", start: 4, own: false },
    ];
    expect([1, 2].map((i) => takeNumber(rows, i))).toEqual([1, 2]);
  });
});
