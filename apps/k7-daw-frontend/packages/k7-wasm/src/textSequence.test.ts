// Addressing ONE note in a sequence and rewriting it: what the piano roll commits. The file is hand-edited, so the shape is pinned
// as hard as the value: one note per line, the others untouched, no `v(1)` written, a short form kept or written in full.

import { describe, expect, it } from "vitest";
import { parse } from "./k7x/index.ts";
import { editSequence, type SequenceOp, sequenceData } from "./text.ts";
import { validateFile } from "./validate.ts";

const AUTHORED = `from("1:1")
notes([
  t(0).d(1).k(60),
  t(1).d(0.5).k(64).v(0.4),
  t(1).d(0.5).k(67),
])
`;

const edit = (text: string, ...ops: SequenceOp[]): string =>
  editSequence(text, ops).text;
const notesOf = (text: string) =>
  parse(text).notes as Record<string, unknown>[];
const valid = (text: string) => validateFile("sequences/s.k7x", text).ok;

describe("reading a sequence", () => {
  it("reads a short form as the notes it expands to", () => {
    expect(sequenceData('notes("c4 e4")\n').notes).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 1, k: 64 },
    ]);
  });

  it("throws for a text that does not read", () => {
    expect(() => sequenceData("notes([")).toThrow();
  });
});

describe("removeNote", () => {
  it("removes exactly the note addressed and leaves the others alone", () => {
    const after = edit(AUTHORED, { op: "removeNote", k: 64, t: 1 });
    expect(notesOf(after)).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 0.5, k: 67 },
    ]);
    expect(valid(after)).toBe(true);
  });

  it("tells two stacked notes apart by their key", () => {
    const after = edit(AUTHORED, { op: "removeNote", k: 67, t: 1 });
    expect(notesOf(after)).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 0.5, k: 64, v: 0.4 },
    ]);
  });

  it("refuses a note it cannot find, rather than removing something else", () => {
    expect(() => edit(AUTHORED, { op: "removeNote", k: 60, t: 9 })).toThrow(
      /no note/,
    );
  });

  it("keeps everything else in the file", () => {
    expect(edit(AUTHORED, { op: "removeNote", k: 60, t: 0 })).toContain(
      'from("1:1")',
    );
  });
});

describe("setNoteVelocity", () => {
  const velocity = (k: number, t: number, v?: number): SequenceOp => ({
    op: "setNoteVelocity",
    k,
    t,
    v,
  });

  it("writes the velocity and leaves the other notes alone", () => {
    const after = edit(AUTHORED, velocity(60, 0, 0.75));
    expect(notesOf(after)).toEqual([
      { t: 0, d: 1, k: 60, v: 0.75 },
      { t: 1, d: 0.5, k: 64, v: 0.4 },
      { t: 1, d: 0.5, k: 67 },
    ]);
    // One note per line, still, and a knob drag is a one-line diff.
    expect(after).toBe(AUTHORED.replace("k(60)", "k(60).v(0.75)"));
    expect(valid(after)).toBe(true);
  });

  it("replaces a velocity the note already had", () => {
    expect(notesOf(edit(AUTHORED, velocity(64, 1, 0.9)))[1]).toEqual({
      t: 1,
      d: 0.5,
      k: 64,
      v: 0.9,
    });
  });

  it("CLEARS the key rather than writing the default", () => {
    const after = edit(AUTHORED, velocity(64, 1));
    expect(notesOf(after)[1]).toEqual({ t: 1, d: 0.5, k: 64 });
    expect(after).not.toContain("v(");
  });

  it("refuses a note it cannot find, rather than writing somewhere else", () => {
    expect(() => edit(AUTHORED, velocity(60, 9, 0.5))).toThrow(/no note/);
  });

  it("keeps everything else in the file, comments included", () => {
    const withComments = `// the melody
notes([
  // the downbeat
  t(0).d(1).k(60),
])
`;
    const after = edit(withComments, velocity(60, 0, 0.5));
    expect(after).toContain("// the melody");
    expect(after).toContain("// the downbeat");
  });
});

describe("a sequence written in the short form", () => {
  it("keeps the short form while an edit leaves its notes as they were", () => {
    const text = 'notes("c4 e4")\n';
    expect(
      edit(text, { op: "setNoteVelocity", k: 60, t: 0, v: undefined }),
    ).toBe(text);
  });

  it("writes the notes in full once an edit changes them", () => {
    const after = edit('notes("c4 e4")\n', {
      op: "setNoteVelocity",
      k: 64,
      t: 1,
      v: 0.5,
    });
    expect(notesOf(after)).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 1, k: 64, v: 0.5 },
    ]);
  });

  it("answers an unreadable text as an Error and a refused op as its sentence", () => {
    expect(() => edit("notes([", { op: "removeNote", k: 1, t: 0 })).toThrow();
  });
});

describe("setNotes", () => {
  it("writes the whole list changing only the notes that changed, a pad's effects carried by the caller", () => {
    const next = [
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 0.5, k: 64, v: 0.9 },
      { t: 1, d: 0.5, k: 67 },
    ];
    expect(edit(AUTHORED, { op: "setNotes", notes: next })).toBe(
      AUTHORED.replace("v(0.4)", "v(0.9)"),
    );
  });

  it("keeps a short form that the new notes still say", () => {
    const text = 'notes("c4 e4")\nto("2:1")\n';
    const same = [
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 1, k: 64 },
    ];
    expect(edit(text, { op: "setNotes", notes: same })).toBe(text);
  });

  it("reports the splices it made, in UTF-16 units of the text before", () => {
    const out = editSequence(AUTHORED, [
      {
        op: "setNotes",
        notes: [
          { t: 0, d: 1, k: 60 },
          { t: 1, d: 0.5, k: 64, v: 0.9 },
          { t: 1, d: 0.5, k: 67 },
        ],
      },
    ]);
    expect(out.changes).toHaveLength(1);
    const [c] = out.changes;
    expect(AUTHORED.slice(0, c.start) + c.insert + AUTHORED.slice(c.end)).toBe(
      out.text,
    );
  });
});

// Moving a note to another pad keeps its length and its velocity: it is the same note.
describe("moveNote", () => {
  const move = (k: number, t: number, newK: number, newT: number) =>
    edit(AUTHORED, { op: "moveNote", k, t, newK, newT });

  it("carries the note's length and velocity to the new key and start", () => {
    const after = move(64, 1, 60, 2.5);
    expect(notesOf(after)).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 2.5, d: 0.5, k: 60, v: 0.4 },
      { t: 1, d: 0.5, k: 67 },
    ]);
    expect(valid(after)).toBe(true);
  });

  it("moves within one key, and to another key at the same beat", () => {
    expect(notesOf(move(60, 0, 60, 3))[0]).toEqual({ t: 3, d: 1, k: 60 });
    expect(notesOf(move(60, 0, 62, 0))[0]).toEqual({ t: 0, d: 1, k: 62 });
  });

  it("refuses a destination that already holds one: (key, start) is the address", () => {
    expect(() => move(60, 0, 67, 1)).toThrow(/already/);
  });

  it("accepts a move that lands exactly where it started", () => {
    expect(notesOf(move(60, 0, 60, 0))[0]).toEqual({ t: 0, d: 1, k: 60 });
  });

  it("refuses a note it cannot find, and a start before the sequence", () => {
    expect(() => move(60, 9, 61, 0)).toThrow(/no note/);
    expect(() => move(60, 0, 61, -1)).toThrow(/before/);
  });
});

describe("moveNotes", () => {
  const moving = (moves: { k: number; t: number; nk: number; nt: number }[]) =>
    edit(AUTHORED, { op: "moveNotes", moves });

  it("moves every note in one pass, each to its own place, length and velocity riding along", () => {
    expect(
      notesOf(
        moving([
          { k: 60, t: 0, nk: 62, nt: 2 },
          { k: 64, t: 1, nk: 65, nt: 3 },
        ]),
      ),
    ).toEqual([
      { t: 2, d: 1, k: 62 },
      { t: 3, d: 0.5, k: 65, v: 0.4 },
      { t: 1, d: 0.5, k: 67 },
    ]);
  });

  it("lands on a place another MOVING note leaves: a selection shifted onto itself", () => {
    expect(
      notesOf(
        moving([
          { k: 64, t: 1, nk: 67, nt: 1 },
          { k: 67, t: 1, nk: 67, nt: 2 },
        ]),
      ),
    ).toEqual([
      { t: 0, d: 1, k: 60 },
      { t: 1, d: 0.5, k: 67, v: 0.4 },
      { t: 2, d: 0.5, k: 67 },
    ]);
  });

  it("refuses what it cannot do, writing nothing", () => {
    expect(() => moving([{ k: 64, t: 1, nk: 60, nt: 0 }])).toThrow(/already/);
    expect(() =>
      moving([
        { k: 60, t: 0, nk: 70, nt: 4 },
        { k: 64, t: 1, nk: 70, nt: 4 },
      ]),
    ).toThrow(/two notes/);
    expect(() => moving([{ k: 61, t: 0, nk: 62, nt: 0 }])).toThrow(/no note/);
    expect(() => moving([{ k: 60, t: 0, nk: 60, nt: -1 }])).toThrow(/before/);
    expect(() => moving([])).toThrow(/no note/);
  });
});

describe("setNoteTimeRange", () => {
  const SEQ = `notes([
  t(0).d(1).k(60),
  t(4).d(2).k(64).v(0.7),
])
`;
  const range = (k: number, t: number, newT: number, newD: number) =>
    edit(SEQ, { op: "setNoteTimeRange", k, t, newT, newD });

  it("moves a note keeping the length and everything else it carries", () => {
    expect(notesOf(range(64, 4, 6, 2))[1]).toEqual({
      t: 6,
      d: 2,
      k: 64,
      v: 0.7,
    });
    expect(range(64, 4, 6, 2)).toContain("  t(0).d(1).k(60),\n");
  });

  it("stretches a note keeping the start", () => {
    expect(notesOf(range(60, 0, 0, 3))[0]).toEqual({ t: 0, d: 3, k: 60 });
  });

  it("refuses what it cannot write", () => {
    expect(() => range(60, 99, 0, 1)).toThrow(/k60.*99/);
    expect(() => range(60, 0, 0, 0)).toThrow(/longer than/);
    expect(() => range(60, 0, 0, -1)).toThrow(/longer than/);
    expect(() => range(60, 0, -1, 1)).toThrow(/before/);
  });
});

describe("splitNote", () => {
  const SEQ = `notes([
  t(0).d(1).k(60),
  t(4).d(2).k(64).v(0.7),
])
`;
  const split = (k: number, t: number, at: number) =>
    edit(SEQ, { op: "splitNote", k, t, at });

  it("shortens the first half and writes the second, VELOCITY carried onto both", () => {
    expect(notesOf(split(64, 4, 5)).slice(1)).toEqual([
      { t: 4, d: 1, k: 64, v: 0.7 },
      { t: 5, d: 1, k: 64, v: 0.7 },
    ]);
    expect(split(64, 4, 5)).toContain("  t(0).d(1).k(60),\n");
  });

  it("leaves a note with no velocity without one", () => {
    const notes = notesOf(split(60, 0, 0.25));
    expect(notes[0]).toEqual({ t: 0, d: 0.25, k: 60 });
    expect(notes[2]).toEqual({ t: 0.25, d: 0.75, k: 60 });
  });

  it("refuses a missing note and a cut on or outside an edge, naming where the note runs", () => {
    expect(() => split(60, 99, 99.5)).toThrow(/k60/);
    for (const at of [4, 6, 9, 1])
      expect(() => split(64, 4, at)).toThrow(/inside the note/);
    expect(() => split(64, 4, 6)).toThrow(/4 to 6/);
  });
});

describe("doubleSequence", () => {
  const PLAIN = `notes([
  t(0).d(1).k(60),
  t(2.5).d(0.5).k(64).v(0.8),
])
`;
  const double = (text: string, num: number, den: number): string =>
    edit(text, { op: "doubleSequence", num, den });

  it("copies every note one BAR-ROUNDED extent later, writing no window", () => {
    expect(notesOf(double(PLAIN, 4, 4)).slice(2)).toEqual([
      { t: 4, d: 1, k: 60 },
      { t: 6.5, d: 0.5, k: 64, v: 0.8 },
    ]);
    expect(parse(double(PLAIN, 4, 4)).to).toBeUndefined();
    expect(double(PLAIN, 4, 4)).toContain("  t(4).d(1).k(60),\n");
  });

  it("reads the bar off the SIGNATURE, so 3/4 doubles by three beats", () => {
    expect(notesOf(double(PLAIN, 3, 4))[2]).toEqual({ t: 3, d: 1, k: 60 });
  });

  const WINDOWED = `from("1:1")
to("3:1")
notes([
  t(0).d(1).k(60).repeat(2),
  t(9).d(1).k(62),
])
`;

  it("moves a stated `to` out by the window's length, copying what the window HOLDS with its pad effects", () => {
    expect(parse(double(WINDOWED, 4, 4)).to).toBe("5:1");
    const notes = notesOf(double(WINDOWED, 4, 4));
    expect(notes[2]).toEqual({ t: 8, d: 1, k: 60, repeat: 2 });
    expect(notes.filter((n) => n.k === 62)).toHaveLength(1);
    expect(notes.filter((n) => n.repeat === 2)).toHaveLength(2);
  });

  it("refuses an empty sequence and a window with nothing in it", () => {
    expect(() => double("notes([])\n", 4, 4)).toThrow(/no notes/);
    expect(() =>
      double('from("1:1")\nto("2:1")\nnotes([t(8).d(1).k(60)])\n', 4, 4),
    ).toThrow(/nothing to double/);
  });
});

describe("appendNotes", () => {
  it("appends each note on a line of its own, a pad's effects carried, the list made when absent", () => {
    const out = edit('from("1:1")\n', {
      op: "appendNotes",
      notes: [{ t: 0, d: 1, k: 60, repeat: 2 }],
    });
    expect(notesOf(out)).toEqual([{ t: 0, d: 1, k: 60, repeat: 2 }]);
    const more = edit(AUTHORED, {
      op: "appendNotes",
      notes: [{ t: 4, d: 1, k: 72, v: 0.5 }],
    });
    expect(more).toContain("  t(4).d(1).k(72).v(0.5),\n");
  });
});

describe("setNoteEffect", () => {
  it("turns a pad effect on, off, and refuses a count that says nothing", () => {
    const on = edit(AUTHORED, {
      op: "setNoteEffect",
      k: 60,
      t: 0,
      effect: "repeat",
      n: 3,
    });
    expect(notesOf(on)[0]).toEqual({ t: 0, d: 1, k: 60, repeat: 3 });
    const off = edit(on, {
      op: "setNoteEffect",
      k: 60,
      t: 0,
      effect: "repeat",
    });
    expect(off).toBe(AUTHORED);
    expect(() =>
      edit(AUTHORED, {
        op: "setNoteEffect",
        k: 60,
        t: 0,
        effect: "skip",
        n: 1,
      }),
    ).toThrow(/whole number of 2 or more/);
  });
});
