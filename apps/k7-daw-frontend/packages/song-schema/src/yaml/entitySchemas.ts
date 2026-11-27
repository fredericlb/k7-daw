// entitySchemas.ts — what a clip's source names, and one note's shape for the roll and the timeline's types.
// A reader would get wrong that nothing here validates a clip, sequence or curve file: that is the Rust reader's, through `file.validate`.

import { z } from "zod";

/** A clip is PICTURE when its source ends in one of these — the whole reason a clip file can say only `source:`. */
export const VIDEO_SOURCE = /\.(mp4|mov|m4v|webm|mkv)$/i;

/** …and a STILL when it ends in one of these: a picture with no time of its own (song-video docs/media.md). */
export const IMAGE_SOURCE = /\.(png|jpe?g|webp|gif|avif)$/i;

/** …and CAPTIONS when it is a subtitle file: timed text, drawn on a picture lane (song-video docs/titles.md). */
export const CAPTION_SOURCE = /\.(srt|vtt)$/i;

/** One note — and a PAD when it carries effects, which is why two effects live here (docs/padEffects.md). */
export const NoteSchema = z
  .object({
    t: z.number().min(0), // onset, beats (fractional = microtiming)
    d: z.number().positive(), // duration, beats
    k: z.number().int().min(0).max(127), // MIDI key
    v: z.number().min(0).max(1).optional(), // velocity, default 1
    // Retrigger: play the note this many times inside its OWN duration; absent == once.
    repeat: z.number().int().min(2).optional(),
    // Fall silent on every n-th pass of the pattern; absent == every pass (docs/entityFiles.md).
    skip: z.number().int().min(2).optional(),
  })
  .strict();
