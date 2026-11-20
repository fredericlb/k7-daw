// peakChain.ts — the pure half of the peak pass: which chains want a peak and how it is written in.
// Nothing here decodes: the verbs' Worker imports it, and the file's peak is the host's `peak` member.

import type { PluginDoc } from "@studio/song/project";

/** The param a MEASURING cold module receives its number in — one name, spelled once. */
const PEAK_PARAM = "Peak";

/** Does anything in this chain want a peak measured? Asked of the COMPILED plugins, docs/peakPass.md. */
export function needsPeak(plugins: readonly PluginDoc[]): boolean {
  return plugins.some((p) => PEAK_PARAM in (p.params ?? {}));
}

/** The same plugins with the measurement written in — a COPY, never a mutation; docs/peakPass.md. */
export function withMeasuredPeak(
  plugins: readonly PluginDoc[],
  peakDb: number,
): PluginDoc[] {
  return plugins.map((p) =>
    PEAK_PARAM in (p.params ?? {})
      ? { ...p, params: { ...p.params, [PEAK_PARAM]: peakDb } }
      : p,
  );
}
