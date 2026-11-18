// peakPass.ts — the measuring half of a COLD module: read a file's true peak and write it into the
// param the module reads it from, immediately before the render. Without it a normaliser applies
// its default gain instead of normalising. Full rationale: docs/peakPass.md.

import { measureFilePeak } from "@studio/audio-analysis/peak";

export { needsPeak, PEAK_PARAM, withMeasuredPeak } from "./peakChain";

/** One file's true peak in dBFS, decoded by the engine and NOT read off the sidecar — docs/peakPass.md. */
export async function measureFilePeakDb(
  engineBin: string,
  source: string,
): Promise<number> {
  try {
    return (await measureFilePeak(source, { engineBin })).peakDb;
  } catch (e) {
    throw new Error(
      `could not measure the peak of ${source}: ${(e as Error).message}`,
    );
  }
}
