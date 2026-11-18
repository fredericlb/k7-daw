// latestWriter.ts — a control's writes, one in flight at a time, and only the newest of those that came meanwhile.
// What a reader gets wrong: nothing is dropped that a person would keep — a later line names the same param. See ../../docs/mastering.md.

/** Wraps `send` so a line arriving while one is in flight replaces any line already waiting. */
export function latestWriter(
  send: (line: string) => unknown,
): (line: string) => void {
  let busy = false;
  let waiting: string | undefined;
  const pump = async (line: string): Promise<void> => {
    busy = true;
    try {
      await send(line);
    } catch {
      // The sender reports its own failure; the next line still goes.
    } finally {
      busy = false;
    }
    const next = waiting;
    waiting = undefined;
    if (next !== undefined) void pump(next);
  };
  return (line) => {
    if (busy) waiting = line;
    else void pump(line);
  };
}
