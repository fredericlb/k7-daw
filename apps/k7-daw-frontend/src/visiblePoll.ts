// visiblePoll.ts — k7's poll: the poll's, its first tick one period from now unless asked;
// and pollUntil, a job followed to its end. A background tab asks nothing to pollWhileVisible, and its
// return asks once at once (poll.ts); pollUntil keeps asking in a background tab.

import { poll } from "./poll";

/** Run `fn` `ms` after each run ends while the tab is visible, and once on its return; the result stops both. */
export const pollWhileVisible = (
  fn: () => unknown,
  ms: number | (() => number),
  opts: { immediate?: boolean } = {},
): (() => void) => poll(fn, ms, { immediate: opts.immediate ?? false });

/** How pollUntil paces itself and what it shows while the job runs. */
export interface PollUntilOptions<T> {
  /** The wait between one answer and the next read. */
  everyMs: number;
  /** Each answer that is not yet done, before the wait. */
  onEach?: (v: T) => void;
  /** Asked after each wait: true ends the poll on the last answer, unread. */
  stopped?: () => boolean;
  /** Past `ms` from the start, the wait is followed by `error()` thrown instead of a read. */
  deadline?: { ms: number; error: () => Error };
}

/** Read `first`, then `read(last)` every `everyMs`, until `isDone`; answers the last value read. */
export async function pollUntil<T>(
  first: T | Promise<T>,
  read: (last: T) => Promise<T>,
  isDone: (v: T) => boolean,
  opts: PollUntilOptions<T>,
): Promise<T> {
  const until = opts.deadline ? Date.now() + opts.deadline.ms : Infinity;
  let v = await first;
  while (!isDone(v)) {
    opts.onEach?.(v);
    await new Promise((r) => setTimeout(r, opts.everyMs));
    if (opts.stopped?.()) return v;
    if (opts.deadline && Date.now() >= until) throw opts.deadline.error();
    v = await read(v);
  }
  return v;
}
