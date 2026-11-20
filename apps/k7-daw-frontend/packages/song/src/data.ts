// data.ts — the two guards a reader of parsed file data narrows `unknown` with.
// What a reader gets wrong: an array is not a record, and a missing list reads as empty.

/** A plain object — never null, never an array. */
export const isRecord = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v);

/** `v` when it is a list, else an empty one. */
export const listOf = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
