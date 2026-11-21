import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BEAT_MS, startPresence } from "./presence";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function page() {
  const listeners = new Map<string, () => void>();
  return {
    addEventListener: (t: string, f: () => void) => void listeners.set(t, f),
    removeEventListener: (t: string) => void listeners.delete(t),
    fire: (t: string) => listeners.get(t)?.(),
    has: (t: string) => listeners.has(t),
  };
}

describe("startPresence", () => {
  it("beats at once and every five seconds", async () => {
    const beat = vi.fn();
    const stop = startPresence({ beat, bye: vi.fn(), target: page() as never });
    expect(beat).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(BEAT_MS);
    expect(beat).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(BEAT_MS * 2);
    expect(beat).toHaveBeenCalledTimes(4);
    stop();
  });

  it("says goodbye on pagehide, and again on the stop, then beats no more", async () => {
    const beat = vi.fn();
    const bye = vi.fn();
    const target = page();
    const stop = startPresence({ beat, bye, target: target as never });
    target.fire("pagehide");
    expect(bye).toHaveBeenCalledTimes(1);
    stop();
    expect(bye).toHaveBeenCalledTimes(2);
    expect(target.has("pagehide")).toBe(false);
    await vi.advanceTimersByTimeAsync(BEAT_MS * 3);
    expect(beat).toHaveBeenCalledTimes(1);
  });

  it("goes on beating after a beat that failed", async () => {
    const beat = vi.fn().mockRejectedValue(new Error("down"));
    const stop = startPresence({ beat, bye: vi.fn(), target: page() as never });
    await vi.advanceTimersByTimeAsync(BEAT_MS);
    expect(beat).toHaveBeenCalledTimes(2);
    stop();
  });
});
