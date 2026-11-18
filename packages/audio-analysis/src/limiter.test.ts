import { describe, expect, it } from "vitest";
import { createLimiter, TranscodeBusyError } from "./limiter";

const later = (ms: number) => new Promise((r) => setTimeout(r, ms));

describe("createLimiter", () => {
  it("runs N tasks with at most K in flight, all of them to the end", async () => {
    const limiter = createLimiter(2);
    let inFlight = 0;
    let peak = 0;
    const results = await Promise.all(
      Array.from({ length: 7 }, (_, i) =>
        limiter.run(async () => {
          inFlight++;
          peak = Math.max(peak, inFlight);
          await later(10);
          inFlight--;
          return i;
        }),
      ),
    );
    expect(peak).toBe(2);
    expect(results).toEqual([0, 1, 2, 3, 4, 5, 6]);
    expect(limiter.active).toBe(0);
    expect(limiter.waiting).toBe(0);
  });

  it("refuses a task past its waiting bound, and takes one again once the queue moved", async () => {
    const limiter = createLimiter(1, 1);
    let open: () => void = () => {};
    const first = limiter.run(() => new Promise<void>((r) => (open = r)));
    const second = limiter.run(async () => "second");
    await expect(limiter.run(async () => "third")).rejects.toBeInstanceOf(
      TranscodeBusyError,
    );
    open();
    await first;
    expect(await second).toBe("second");
    expect(await limiter.run(async () => "fourth")).toBe("fourth");
  });

  it("frees a slot when a task fails", async () => {
    const limiter = createLimiter(1);
    await expect(
      limiter.run(async () => {
        throw new Error("no");
      }),
    ).rejects.toThrow("no");
    expect(await limiter.run(async () => "next")).toBe("next");
  });

  it("starts waiting tasks as soon as it is widened", async () => {
    const limiter = createLimiter(1);
    let open: () => void = () => {};
    const first = limiter.run(() => new Promise<void>((r) => (open = r)));
    const second = limiter.run(async () => "second");
    expect(limiter.waiting).toBe(1);
    limiter.max = 2;
    expect(limiter.waiting).toBe(0);
    expect(await second).toBe("second");
    open();
    await first;
  });
});
