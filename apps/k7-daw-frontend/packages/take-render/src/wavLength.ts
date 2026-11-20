// wavLength.ts — how long a wav is, read from its header through the chunk walk of wav.ts.
// What reads wrong: a 0 `data` size is 0 s here, not what is on disk as for a slice (docs/rendering.md).

import { closeSync, fstatSync, openSync, readSync } from "node:fs";
import { wavLayout } from "./wav.ts";

/** As much of a header as any sane file spends before `data` — a `LIST`/`bext` block and change. */
const HEAD_BYTES = 8192;

/** The duration of a PCM or float RIFF/WAVE file in seconds, or NULL when it cannot be read — docs/rendering.md. */
export function wavSeconds(path: string): number | null {
  let fd: number | null = null;
  try {
    fd = openSync(path, "r");
    const buf = Buffer.alloc(HEAD_BYTES);
    const read = readSync(fd, buf, 0, HEAD_BYTES, 0);
    // Through the SAME descriptor, so a replaced file cannot mix one file's size with another's bytes.
    const size = fstatSync(fd).size;
    const w = wavLayout(buf.subarray(0, read), size);
    if (!w) return null;
    const onDisk = Math.max(0, size - w.dataOffset);
    return Math.min(w.declaredBytes, onDisk) / (w.blockAlign * w.sampleRate);
  } catch {
    return null;
  } finally {
    if (fd !== null)
      try {
        closeSync(fd);
      } catch {
        /* already gone */
      }
  }
}
