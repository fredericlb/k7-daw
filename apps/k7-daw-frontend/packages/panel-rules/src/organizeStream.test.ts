// The run's stream as the page reads it: progress lines, then the panel or the error; a line that is no event is skipped.

import { describe, expect, it } from "vitest";
import { readOrganizeStream } from "./organizeStream";

const stream = (...chunks: string[]): ReadableStream<Uint8Array> =>
  new ReadableStream({
    start(c) {
      for (const s of chunks) c.enqueue(new TextEncoder().encode(s));
      c.close();
    },
  });

const RESULT = { uiText: "cols(1)\n", widgetsJs: "", notes: "n" };

describe("reading a run", () => {
  it("passes the progress on and answers the panel, whatever the chunks split", async () => {
    const seen: string[] = [];
    const line = JSON.stringify({ type: "done", result: RESULT });
    const got = await readOrganizeStream(
      stream(
        '{"type":"status","text":"a"}\n{"type":"pi',
        'ng"}\nnot json\n{"type":"mystery"}\n{"type":"status","text":"b"}\n',
        line.slice(0, 10),
        `${line.slice(10)}\n`,
      ),
      (t) => seen.push(t),
    );
    expect(seen).toEqual(["a", "b"]);
    expect(got).toEqual(RESULT);
  });

  it("throws the host's error, and a stream that stops before either end", async () => {
    await expect(
      readOrganizeStream(
        stream('{"type":"error","error":"Claude declined"}\n'),
        () => {},
      ),
    ).rejects.toThrow("Claude declined");
    await expect(
      readOrganizeStream(stream('{"type":"status","text":"a"}\n'), () => {}),
    ).rejects.toThrow("the answer stopped before it was complete");
  });
});
