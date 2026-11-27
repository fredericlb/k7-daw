// placementId.ts — a clip's id: the material it plays, and WHERE it plays. Three functions and no
// imports, so a BROWSER can hold the rule: it is written by the compiler, logged by the engine,
// carried on `/waveform`, and read back by every surface that has to find the file behind a clip.

/** A placement's id — the material, `@`, and the start IN BEATS; readable on purpose (docs/laneEdits.md). */
export function placementId(ref: string, startBeats: number): string {
  return `${ref}@${startBeats}`;
}

/** `id` without the `#<n>` the compiler adds to a placement repeated at one beat of one lane (docs/laneEdits.md). */
export function withoutRepeat(id: string): string {
  return id.replace(/(.)#\d+$/, "$1");
}

/** The start in beats a placement id names, its repeat's `#n` aside; NaN when it names none. */
export function placementStartOf(id: string): number {
  const clipId = withoutRepeat(id.slice(id.lastIndexOf(">") + 1));
  const at = clipId.lastIndexOf("@");
  const tail = clipId.slice(at + 1);
  return at > 0 && tail !== "" ? Number(tail) : Number.NaN;
}
