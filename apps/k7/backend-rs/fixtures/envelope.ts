// envelope.ts — the one answer shape the settle's referee and the text vectors give Rust: `{ok}` with the value, or
// `{err}` with the error's class name and message.

export type Args = Record<string, unknown>;

/** An error as the envelope carries it: the class name, which Rust maps, and the message. */
export const failure = (e: unknown) => ({
  err: {
    name: (e as Error)?.name ?? "Error",
    message: (e as Error)?.message ?? String(e),
  },
});

/** One operation of `table`, answered as the envelope Rust reads. */
export function answer(
  table: Record<string, (a: Args) => unknown>,
  op: string,
  args: string,
  fail: (e: unknown) => ReturnType<typeof failure> = failure,
): string {
  const fn = Object.hasOwn(table, op) ? table[op] : undefined;
  if (!fn) return JSON.stringify(failure(new Error(`no operation ${op}`)));
  try {
    return JSON.stringify({ ok: fn(JSON.parse(args)) ?? null });
  } catch (e) {
    return JSON.stringify(fail(e));
  }
}
