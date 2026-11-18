// downloadSignal.test.ts — the panel's wait for the tracks route's first byte: started, refused, or silent.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { awaitStart } from "./downloadSignal";

let jar = "";
const read = () => jar;
const set = (name: string, value: string) => {
  jar = `other=1; k7dl-${name}=${value}`;
};

describe("awaitStart", () => {
  beforeEach(() => {
    jar = "";
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it("holds until the cookie appears, then says started ", async () => {
    let result: unknown = null;
    void awaitStart("a1", { jar: read }).then((r) => {
      result = r;
    });
    await vi.advanceTimersByTimeAsync(1000);
    expect(result).toBeNull();
    set("a1", "ok");
    await vi.advanceTimersByTimeAsync(300);
    expect(result).toEqual({ kind: "started" });
  });

  it("carries a refusal's sentence to the caller", async () => {
    set("b2", "err.no%20engine%3B%20%C3%A9");
    expect(await awaitStart("b2", { jar: read })).toEqual({
      kind: "refused",
      message: "no engine; é",
    });
  });

  it("gives up as silent after its bound", async () => {
    const waiting = awaitStart("c3", {
      pollMs: 100,
      timeoutMs: 1000,
      jar: read,
    });
    await vi.advanceTimersByTimeAsync(1200);
    expect(await waiting).toEqual({ kind: "silent" });
  });
});
