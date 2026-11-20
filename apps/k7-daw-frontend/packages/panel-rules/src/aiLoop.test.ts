// The browser's loop against `organize-vectors.json`, the reference as committed: the TypeScript route that
// wrote it ran this very conversation on the server. The same model replies must draw the same turns, the same
// repair message and the same end — and the cases the loop alone decides (junk answers) are here too.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { type AiTurnCall, type AiTurnRequest, organizeLoop } from "./aiLoop";
import { readTurnStream } from "./aiTurnStream";

interface Case {
  name: string;
  request: Record<string, unknown>;
  replies: { status: number; body: string }[];
  upstream: { body: string }[];
  answer: string;
}

const vectors = JSON.parse(
  readFileSync(
    join(
      import.meta.dirname,
      "../../../../k7-daw-backend/fixtures/organize-vectors.json",
    ),
    "utf8",
  ),
) as { cases: Case[] };

interface Folded {
  stop: string | null;
  text: string;
  content: Record<string, unknown>[];
}

/** One streamed reply folded into its message, `null` for a stream that did not reach `message_stop`. */
function fold(body: string): Folded | null {
  const content: Record<string, unknown>[] = [];
  let stop: string | null = null;
  let sealed = false;
  for (const line of body.split("\n")) {
    if (!line.startsWith("data: ")) continue;
    const e = JSON.parse(line.slice(6));
    if (e.type === "content_block_start") content[e.index] = e.content_block;
    else if (e.type === "content_block_delta") {
      const b = content[e.index];
      const d = e.delta;
      if (d.type === "text_delta") b.text += d.text;
      if (d.type === "thinking_delta") b.thinking += d.thinking;
      if (d.type === "signature_delta") b.signature = d.signature;
    } else if (e.type === "message_delta") stop = e.delta.stop_reason;
    else if (e.type === "message_stop") sealed = true;
  }
  if (!sealed) return null;
  const text = content
    .filter((b) => b.type === "text")
    .map((b) => b.text)
    .join("");
  return { stop, text, content };
}

const lines = (c: Case) =>
  c.answer
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

/** The host the vectors stand for: a turn answers the next sealed message, or fails with what the route said. */
function host(c: Case) {
  const said = lines(c).filter((l) => l.type === "error" && !l.issues);
  const messages = c.replies
    .filter((r) => r.status === 200)
    .map((r) => fold(r.body))
    .filter((m) => m !== null);
  const seen: AiTurnRequest[] = [];
  const turn: AiTurnCall = async (req) => {
    seen.push(req);
    const m = messages[seen.length - 1];
    if (!m || m.stop === "refusal" || m.stop === "max_tokens")
      throw new Error(said.at(-1)?.error);
    return { text: m.text, content: m.content };
  };
  return { turn, seen };
}

describe("the loop against the TypeScript route's vectors", () => {
  it("has the cases", () =>
    expect(vectors.cases.length).toBeGreaterThanOrEqual(16));

  for (const c of vectors.cases) {
    it(c.name, async () => {
      const { turn, seen } = host(c);
      const statuses: string[] = [];
      const end = await organizeLoop(c.request, turn, (t) =>
        statuses.push(t),
      ).then(
        (result) => ({ type: "done", result }),
        (e: Error) => ({ type: "error", message: e.message }),
      );
      const route = lines(c);
      // The turns asked for are the requests the route made upstream, a retry of one counted once.
      const upstream = c.upstream
        .map((u) => JSON.parse(u.body))
        .filter(
          (b, i, all) =>
            i === 0 || all[i - 1].messages.length !== b.messages.length,
        );
      expect(seen.length).toBe(upstream.length);
      seen.forEach((s, i) => {
        expect(s.system).toBe(upstream[i].system[0].text);
        expect(s.messages).toEqual(upstream[i].messages);
        expect(s.schema).toEqual(upstream[i].output_config.format.schema);
      });
      expect(statuses).toEqual(
        route
          .filter((l) => l.type === "status" && /^Claude is /.test(l.text))
          .map((l) => l.text),
      );
      const last = route.at(-1);
      if (c.request.mode === "update" && seen.length === 0 && !last.type) {
        // The route's 400 body: `{ error }`, which the loop throws as its message.
        expect(end).toEqual({ type: "error", message: last.error });
      } else if (last.type === "done") {
        expect(end).toEqual({ type: "done", result: last.result });
      } else {
        const said = [last.error, ...(last.issues ?? [])].join(" — ");
        expect(end).toEqual({ type: "error", message: said });
      }
    });
  }
});

const REQUEST = {
  module: "Warm",
  moduleText: "",
  params: [{ name: "Gain", meta: { min: 0, max: 1 } }],
  uiText: "",
  widgetsJs: "",
  mode: "reset",
  direction: "",
};

const answer = (over: Record<string, unknown> = {}) =>
  JSON.stringify({
    header: "Warm",
    panel: {
      cols: 4,
      rows: 2,
      widgets: [
        {
          id: "gain",
          type: "knob",
          col: 0,
          row: 0,
          colSpan: 2,
          rowSpan: 2,
          bind: "Gain",
          min: 0,
          max: 1,
        },
      ],
    },
    widgets_js: "",
    notes: "",
    ...over,
  });

/** A host that answers these texts, one per turn. */
const saying =
  (...texts: unknown[]): AiTurnCall =>
  async () => ({ text: texts.shift() as string, content: [] });

describe("what the loop decides alone", () => {
  it("accepts a good panel", async () => {
    const r = await organizeLoop(REQUEST, saying(answer()), () => {});
    expect(r.uiText).toContain("gain");
  });

  it("refuses a widgets.js that does not compile, and asks once more", async () => {
    const broken = answer({
      panel: {
        cols: 4,
        rows: 2,
        widgets: [
          {
            id: "s",
            type: "lcdscreen",
            col: 0,
            row: 0,
            colSpan: 4,
            rowSpan: 2,
          },
        ],
      },
      widgets_js: "export default { s( }",
    });
    const asked: AiTurnRequest[] = [];
    const turn: AiTurnCall = async (t) => {
      asked.push(t);
      return { text: broken, content: [] };
    };
    await expect(organizeLoop(REQUEST, turn, () => {})).rejects.toThrow(
      /did not pass the checks — .*does not compile/,
    );
    expect(asked).toHaveLength(2);
    expect(asked[1].messages.at(-1)?.content).toMatch(
      /^The panel was refused:\n- /,
    );
  });

  it("compiles a script and never runs it", async () => {
    const hostile = answer({
      panel: {
        cols: 4,
        rows: 2,
        widgets: [
          {
            id: "s",
            type: "lcdscreen",
            col: 0,
            row: 0,
            colSpan: 4,
            rowSpan: 2,
          },
        ],
      },
      widgets_js:
        "export default { s(lcd) {}, spin: (() => { while (true) {} })() }",
    });
    // A script that ran would never return; the call ending is the proof.
    await organizeLoop(REQUEST, saying(hostile), () => {}).catch(() => {});
  });

  it("takes a model's junk for a refused answer, never a crash", async () => {
    const junk: unknown[] = [
      "",
      "not json",
      "null",
      "[]",
      "42",
      '"text"',
      "{}",
      '{"panel":null,"widgets_js":""}',
      '{"panel":[],"widgets_js":""}',
      '{"panel":{"cols":"x","rows":null,"widgets":"no"},"widgets_js":""}',
      '{"panel":{"cols":4,"rows":4,"widgets":[null,1,"x",[],{"id":5}]},"widgets_js":""}',
      '{"panel":{"cols":1e999,"rows":4,"widgets":[]},"widgets_js":0}',
      answer({ widgets_js: 7 }),
      "\u0000\ud800{",
      "x".repeat(2_000_000),
      undefined,
      null,
      { not: "a string" },
    ];
    for (const text of junk) {
      const asked: unknown[] = [];
      const turn: AiTurnCall = async (t) => {
        asked.push(t);
        return { text: text as string, content: text };
      };
      await expect(
        organizeLoop(REQUEST, turn, () => {}),
        String(text).slice(0, 40),
      ).rejects.toThrow(/did not pass the checks/);
      expect(asked).toHaveLength(2);
    }
  });

  it("reads a header or notes of the wrong type as none", async () => {
    const r = await organizeLoop(
      REQUEST,
      saying(answer({ header: { a: 1 }, notes: [] })),
      () => {},
    );
    expect(r.notes).toBe("");
  });

  it("lets a failed turn end the conversation", async () => {
    const turn: AiTurnCall = async () => {
      throw new Error("Claude declined: not a panel");
    };
    await expect(organizeLoop(REQUEST, turn, () => {})).rejects.toThrow(
      "Claude declined: not a panel",
    );
  });

  it("refuses a request that is not one, before a turn is paid for", async () => {
    let asked = 0;
    const turn: AiTurnCall = async () => {
      asked++;
      return { text: answer(), content: [] };
    };
    for (const body of [
      null,
      7,
      "x",
      {},
      { ...REQUEST, params: [{ name: 3 }] },
      { ...REQUEST, mode: "update", direction: "  " },
    ]) {
      await expect(organizeLoop(body, turn, () => {})).rejects.toThrow();
    }
    expect(asked).toBe(0);
  });
});

const stream = (...chunks: string[]) =>
  new ReadableStream<Uint8Array>({
    start(c) {
      for (const k of chunks) c.enqueue(new TextEncoder().encode(k));
      c.close();
    },
  });

describe("a turn's stream", () => {
  it("passes the status lines on and returns the turn", async () => {
    const seen: string[] = [];
    const r = await readTurnStream(
      stream(
        '{"type":"status","text":"Writing… 2k characters"}\n{"type":"pi',
        'ng"}\n{"type":"turn","text":"{}","content":[1]}\n',
      ),
      (t) => seen.push(t),
    );
    expect(r).toEqual({ text: "{}", content: [1] });
    expect(seen).toEqual(["Writing… 2k characters"]);
  });

  it("throws the host's error", async () => {
    await expect(
      readTurnStream(
        stream('{"type":"error","error":"Claude declined"}\n'),
        () => {},
      ),
    ).rejects.toThrow("Claude declined");
  });

  it("skips junk lines and ends on a stream that stopped", async () => {
    const seen: string[] = [];
    await expect(
      readTurnStream(
        stream(
          'garbage\n\n{"type":"nope"}\n{"type":"status","text":5}\n{"type":"status","text":"ok"}\n[1]\nnull\n',
        ),
        (t) => seen.push(t),
      ),
    ).rejects.toThrow("the answer stopped before it was complete");
    expect(seen).toEqual(["ok"]);
  });
});
