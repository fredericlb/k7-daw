// mixSchema.ts — `mix.k7x`'s shape, in Zod: the desk's CUE SHEET, record by record. Where this
// set drops the needle, keyed by each record's workspace path, as the set list names it.
// Full rationale: docs/mixFile.md.

import { isRecordRef } from "@studio/k7-wasm/records";
import { z } from "zod";

// The ref vocabulary itself is `@studio/k7-wasm/records`; this file is only what the FILE looks like.
const RecordRef = z
  .string()
  .refine(
    isRecordRef,
    "expected a workspace audio file path like records/hells-bells.flac",
  );

/** One song's cue points by BUTTON, positional and in SOURCE seconds — see docs/deskFiles.md. */
export const MARKER_SLOTS = 8;
const Markers = z.array(z.number().min(0).nullable()).max(MARKER_SLOTS);

/** A record this desk has cues for, bare or with its cues; cues only, a tempo being no fact of the set (docs/deskFiles.md). */
const CueEntry = z.union([
  RecordRef,
  z.object({ ref: RecordRef, markers: Markers.optional() }).strict(),
]);

export const MixFileSchema = z
  .object({
    // A SEQUENCE and not a map, duplicates being dropped on read rather than refused (docs/deskFiles.md).
    cues: z.array(CueEntry).optional(),
  })
  .strict();

export type MixFile = z.infer<typeof MixFileSchema>;
