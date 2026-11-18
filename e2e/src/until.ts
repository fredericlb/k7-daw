// until.ts — waiting on what a spec waits for: a question asked again, at expect.poll's pace, until its answer holds.
// It returns the last answer rather than throwing, so the assertion after it says what was wrong (docs/e2e.md).
import { expect } from "@playwright/test";

/** Polled fast at first: most answers come within a few hundred milliseconds, and a sleep would wait out the rest. */
const PACE = [50, 100, 250, 500, 1000];

/** Asks `ask` until `done` holds of its answer, for at most `ms`; the last answer, whether it held or not. */
export async function until<T>(
  ask: () => Promise<T> | T,
  done: (v: T) => boolean,
  ms = 30_000,
): Promise<T> {
  // A timeout of 0 is no timeout to expect.poll: a spent budget asks once instead.
  if (ms <= 0) return ask();
  let last: { v: T } | undefined;
  let fault: unknown;
  await expect
    .poll(
      async () => {
        try {
          last = { v: await ask() };
          return done(last.v);
        } catch (e) {
          fault = e;
          return false;
        }
      },
      { timeout: ms, intervals: PACE },
    )
    .toBe(true)
    .catch(() => undefined);
  if (last) return last.v;
  if (fault) throw fault;
  // The budget ran out during the first question: its answer is the one there is.
  return ask();
}
