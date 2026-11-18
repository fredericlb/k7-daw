// A control's writes: one in flight, the newest of the rest after it (../../docs/mastering.md).

import { describe, expect, it } from "vitest";
import { latestWriter } from "./latestWriter";

/** A sender whose writes end when the test says so. */
function slowSender() {
  const sent: string[] = [];
  const ends: (() => void)[] = [];
  const send = (line: string) =>
    new Promise<void>((ok) => {
      sent.push(line);
      ends.push(ok);
    });
  const finish = async () => {
    ends.shift()?.();
    await new Promise((ok) => setTimeout(ok, 0));
  };
  return { sent, send, finish };
}

describe("latestWriter", () => {
  it("sends the first line at once and only the newest of those that came while it was in flight", async () => {
    const s = slowSender();
    const write = latestWriter(s.send);
    for (const v of [101, 102, 103, 104, 105])
      write(`node param master Width ${v}`);
    expect(s.sent).toEqual(["node param master Width 101"]);
    await s.finish();
    expect(s.sent).toEqual([
      "node param master Width 101",
      "node param master Width 105",
    ]);
    await s.finish();
    expect(s.sent).toHaveLength(2);
  });

  it("goes on after a write that failed", async () => {
    const sent: string[] = [];
    const write = latestWriter(async (line) => {
      sent.push(line);
      if (line === "a") throw new Error("refused");
    });
    write("a");
    write("b");
    await new Promise((ok) => setTimeout(ok, 0));
    expect(sent).toEqual(["a", "b"]);
  });
});
