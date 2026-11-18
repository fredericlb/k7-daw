// copyPath.ts — where a prepared copy of a video lives; pure, so the verbs' Worker imports it.

/** Where a copy of `source` is written: beside it, under a name no clip of its own is likely to hold. */
export const copyPathOf = (
  source: string,
  kind: "optimized" | "proxy",
): string => source.replace(/\.[^./]+$/, `.${kind}.mp4`);
