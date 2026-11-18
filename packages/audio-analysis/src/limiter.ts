// limiter.ts — at most N async jobs in flight, the others waiting in arrival order up to a bound, past which one is refused.
// The pool's transcodes go through one (pool.ts), so a job starts only on a worker of its own (docs/transcode.md).

export interface Limiter {
  run<T>(fn: () => Promise<T>): Promise<T>;
  /** How many may run at once; raising it starts waiting jobs at once. */
  max: number;
  readonly active: number;
  readonly waiting: number;
}

/** Refused at once rather than queued: the re-encodes already waiting are more than the process should hold. */
export class TranscodeBusyError extends Error {
  /** Answered 503 with a Retry-After by an HTTP error handler that reads `status`. */
  readonly status = 503;
  constructor() {
    super("the server is busy re-encoding audio; try again in a moment");
    this.name = "TranscodeBusyError";
  }
}

/** Jobs waiting at once, past which a new one is refused: each holds its upload on disk until it runs. */
const MAX_WAITING = 64;

const width = (n: number) => Math.max(1, Math.floor(n) || 1);

/** At most `max` of the functions handed to `run` in flight at once; the others wait in arrival order, `maxWaiting` at most. */
export function createLimiter(max: number, maxWaiting = MAX_WAITING): Limiter {
  let cap = width(max);
  const queue: (() => void)[] = [];
  let active = 0;
  const drain = () => {
    while (active < cap && queue.length) queue.shift()?.();
  };
  return {
    run<T>(fn: () => Promise<T>): Promise<T> {
      return new Promise<T>((resolve, reject) => {
        const start = () => {
          active++;
          fn()
            .then(resolve, reject)
            .finally(() => {
              active--;
              drain();
            });
        };
        if (active < cap) start();
        else if (queue.length >= maxWaiting) reject(new TranscodeBusyError());
        else queue.push(start);
      });
    },
    get max() {
      return cap;
    },
    set max(n: number) {
      cap = width(n);
      drain();
    },
    get active() {
      return active;
    },
    get waiting() {
      return queue.length;
    },
  };
}
