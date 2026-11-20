// fileStamp.ts — a wall-clock stamp for a filename, shared by a take, a bounce and a verb.
// Pure on purpose: the verbs' Worker imports it, so it touches no node module.

/** A wall-clock stamp for a filename — `260814T194357`, shared with a bounce; docs/rendering.md. */
export function fileStamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return (
    `${p(d.getFullYear() % 100)}${p(d.getMonth() + 1)}${p(d.getDate())}` +
    `T${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`
  );
}
