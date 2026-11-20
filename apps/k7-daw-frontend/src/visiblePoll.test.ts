// visiblePoll.test.ts — the poll waits for a slow tick to END before it counts the next period;
// pollUntil shows each answer but the last, and ends on done, on stop or past its deadline.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { pollUntil, pollWhileVisible } from "./visiblePoll";

describe("pollWhileVisible", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("never starts a tick while the last one is still running", async () => {
    let calls = 0;
    let finish: () => void = () => {};
    const stop = pollWhileVisible(() => {
      calls++;
      return new Promise<void>((r) => {
        finish = r;
      });
    }, 100);
    await vi.advanceTimersByTimeAsync(100);
    expect(calls).toBe(1);
    await vi.advanceTimersByTimeAsync(500);
    expect(calls).toBe(1);
    finish();
    await vi.advanceTimersByTimeAsync(100);
    expect(calls).toBe(2);
    stop();
  });

  it("stops for good", async () => {
    const fn = vi.fn();
    const stop = pollWhileVisible(fn, 100);
    await vi.advanceTimersByTimeAsync(250);
    stop();
    await vi.advanceTimersByTimeAsync(500);
    expect(fn).toHaveBeenCalledTimes(2);
  });
});

describe("pollUntil", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("reads every period until done, showing each answer but the last", async () => {
    const shown: number[] = [];
    const out = pollUntil(
      0,
      async (n) => n + 1,
      (n) => n === 3,
      {
        everyMs: 100,
        onEach: (n) => shown.push(n),
      },
    );
    await vi.advanceTimersByTimeAsync(250);
    expect(shown).toEqual([0, 1, 2]);
    await vi.advanceTimersByTimeAsync(100);
    await expect(out).resolves.toBe(3);
    expect(shown).toEqual([0, 1, 2]);
  });

  it("answers the last value unread once stopped", async () => {
    let stop = false;
    const read = vi.fn(async (n: number) => n + 1);
    const out = pollUntil(0, read, () => false, {
      everyMs: 100,
      stopped: () => stop,
    });
    await vi.advanceTimersByTimeAsync(100);
    stop = true;
    await vi.advanceTimersByTimeAsync(100);
    await expect(out).resolves.toBe(1);
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("throws the deadline's error instead of reading past it", async () => {
    const read = vi.fn(async () => 0);
    const out = pollUntil(0, read, () => false, {
      everyMs: 100,
      deadline: { ms: 250, error: () => new Error("slow") },
    });
    const seen = expect(out).rejects.toThrow("slow");
    await vi.advanceTimersByTimeAsync(300);
    await seen;
    expect(read).toHaveBeenCalledTimes(2);
  });
});
