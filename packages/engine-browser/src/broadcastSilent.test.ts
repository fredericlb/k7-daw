// broadcastSilent.test.ts — a subscription that keeps the mix off the interface says so, and says it
// again when a reconnect replays it: a dropped link must not let a recorder's chain feed back.

import { describe, expect, it } from "vitest";
import { EngineClient } from "./engine";

function client(): { c: EngineClient; sent: object[]; replay: () => void } {
  const c = new EngineClient("ws://never-opened");
  const sent: object[] = [];
  const spy = c as unknown as {
    send: (o: object) => void;
    sendNow: (o: object) => void;
    replay: () => void;
  };
  spy.send = (o) => sent.push(o);
  spy.sendNow = (o) => sent.push(o);
  return { c, sent, replay: () => spy.replay() };
}

describe("a silent broadcast", () => {
  it("asks for silence, and a plain one does not", () => {
    const silent = client();
    silent.c.broadcast(true, { silent: true });
    expect(silent.sent).toEqual([
      { type: "broadcast", on: true, codec: "pcm16", silent: true },
    ]);
    const plain = client();
    plain.c.broadcast(true);
    expect(plain.sent).toEqual([
      { type: "broadcast", on: true, codec: "pcm16" },
    ]);
  });

  it("is replayed whole after a reconnect, and forgotten once off", () => {
    const { c, sent, replay } = client();
    c.broadcast(true, { silent: true });
    sent.length = 0;
    replay();
    expect(sent).toContainEqual({
      type: "broadcast",
      on: true,
      codec: "pcm16",
      silent: true,
    });
    c.broadcast(false);
    c.broadcast(true);
    expect(sent.at(-1)).toEqual({
      type: "broadcast",
      on: true,
      codec: "pcm16",
    });
  });
});
