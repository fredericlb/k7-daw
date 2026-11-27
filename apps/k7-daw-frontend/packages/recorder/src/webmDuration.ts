// webmDuration.ts — writes the duration MediaRecorder leaves out of a WebM: Rust patches the head, this file slices the Blob.
// What reads wrong: only the first bytes are ever read; the rest of the take stays a Blob slice, never copied (../docs/recorder.md).

import { patchWebmHead } from "@studio/k7-wasm/media";
import fixWebmDuration from "fix-webm-duration";

/** Bytes read from the start of the take: the EBML header, the Segment's head and its Info all sit well inside. */
const HEAD_BYTES = 64 * 1024;
/** Past this size a take is never read whole: only the header patch may fix its duration. */
const WHOLE_FILE_FIX_MAX = 32 * 1024 * 1024;

/** The header patch reads 64 kB; the whole-file fix, kept for a layout the patch does not know, only on a short take. */
export async function withDuration(
  blob: Blob,
  durationSec: number,
): Promise<Blob> {
  const ms = durationSec * 1000;
  try {
    const patched = await patchWebmDuration(blob, ms);
    if (patched) return patched;
    if (blob.size <= WHOLE_FILE_FIX_MAX)
      return await fixWebmDuration(blob, ms, { logger: false });
  } catch {
    /* fall back to the unfixed blob */
  }
  return blob;
}

/** The take with its duration written, or null when its layout is not the one this patcher knows. */
export async function patchWebmDuration(
  blob: Blob,
  durationMs: number,
): Promise<Blob | null> {
  const head = new Uint8Array(await blob.slice(0, HEAD_BYTES).arrayBuffer());
  const patched = patchWebmHead(head, durationMs);
  if (!patched) return null;
  if (patched.consumed === 0) return blob;
  return new Blob([patched.bytes as BlobPart, blob.slice(patched.consumed)], {
    type: blob.type,
  });
}
