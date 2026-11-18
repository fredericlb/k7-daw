// poll.ts — a poll that never overlaps itself, and rests while the tab is hidden unless told not to.
// The one thing a reader would get wrong: the NEXT tick is scheduled when this one ENDS, so a slow
// server slows the poll down instead of stacking requests behind it.

export interface PollOptions {
  /** Keep ticking in a hidden tab — for a transport whose end must still advance the queue. */
  whileHidden?: boolean;
  /** Run the first tick now (the default) rather than `everyMs` from now. */
  immediate?: boolean;
}

/** The page's document — undefined where there is none (a test under Node). */
const doc = (): Document | undefined =>
  typeof document === "undefined" ? undefined : document;

/** Run `tick` now and `everyMs` after each run ends, a function read again after each run; the returned function stops it. */
export function poll(
  tick: () => unknown,
  everyMs: number | (() => number),
  opts: PollOptions = {},
): () => void {
  const period = () => (typeof everyMs === "number" ? everyMs : everyMs());
  let stopped = false;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const hidden = () => doc()?.hidden === true;
  const resting = () => !opts.whileHidden && hidden();

  const run = async () => {
    timer = undefined;
    if (stopped || running || resting()) return;
    running = true;
    try {
      await tick();
    } catch {
      // A tick reports its own failure; the poll only keeps time.
    } finally {
      running = false;
    }
    if (!stopped && !resting()) timer = setTimeout(run, period());
  };

  // Back in view: tick at once rather than showing what was true when the tab was hidden.
  const onVisible = () => {
    if (!hidden() && timer === undefined && !running) void run();
  };
  doc()?.addEventListener("visibilitychange", onVisible);
  if (opts.immediate === false) timer = setTimeout(run, period());
  else void run();
  return () => {
    stopped = true;
    clearTimeout(timer);
    doc()?.removeEventListener("visibilitychange", onVisible);
  };
}
