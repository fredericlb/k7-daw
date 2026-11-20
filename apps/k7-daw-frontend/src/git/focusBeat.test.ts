import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { focusBeat, type Page } from "./focusBeat";

let visible = true;
let change: (() => void) | null = null;
const page: Page = {
  visible: () => visible,
  onChange: (cb) => {
    change = cb;
    return () => {
      change = null;
    };
  },
};

beforeEach(() => {
  vi.useFakeTimers();
  visible = true;
});
afterEach(() => vi.useRealTimers());

describe("the focus heartbeat", () => {
  it("says in front at once, and again with each beat", async () => {
    const send = vi.fn();
    focusBeat(send, 5000, page);
    expect(send).toHaveBeenCalledWith(true, true);
    await vi.advanceTimersByTimeAsync(5000);
    await vi.advanceTimersByTimeAsync(5000);
    expect(send.mock.calls.filter((c) => c[0] === true).length).toBe(3);
  });

  it("says away when the page is hidden, and ends the beat on stop", async () => {
    const send = vi.fn();
    const stop = focusBeat(send, 5000, page);
    visible = false;
    change?.();
    expect(send).toHaveBeenLastCalledWith(false, false);
    stop();
    send.mockClear();
    await vi.advanceTimersByTimeAsync(20000);
    expect(send).not.toHaveBeenCalled();
  });

  it("survives a send that fails", async () => {
    const send = vi.fn().mockRejectedValue(new Error("down"));
    focusBeat(send, 5000, page);
    await vi.advanceTimersByTimeAsync(5000);
    expect(send.mock.calls.length).toBeGreaterThan(1);
  });
});
