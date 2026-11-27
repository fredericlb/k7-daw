// timelineDrop.ts — what a drop of OS files onto the timeline is told to the person: the progress it draws and the one line it reports.
// The rules that decide what a drop resolves to (each file's kind, its clip id, its lane) are Rust's: @studio/k7-wasm/intake. See docs/timeline.md.

import type { DropLane, DropPlan } from "@studio/k7-wasm/intake";
import { m } from "./paraglide/messages.js";

/** What `UploadProgress` draws while the bytes go up — the shape the file tree's own upload keeps. */
export interface DropUpload {
  fileName: string;
  loaded: number;
  total: number;
  done: number;
  count: number;
}

/** One drop, as the timeline resolved it: where it landed, in both spellings, and what fell there. */
export interface TimelineDropEvent {
  /** The lane under the pointer — `lanes` is every lane from the top, so the drop can walk down. */
  lane: string;
  lanes: DropLane[];
  /** A timecode for `event add`, seconds for `picture add`: the two verbs' dialects (apps/k7-daw-wasm/verbs/docs/verbs/picture.md). */
  tc: string;
  seconds: number;
  files: File[];
}

/**
 * What the drop WROTE, as one line — said before the placements run, since each of those is a
 * command and reports itself. Every count it could get wrong is named rather than guessed at.
 */
export function dropReport(plan: DropPlan, reused: readonly string[]): string {
  const added = plan.declare.length - reused.length;
  const left = plan.declare.length - plan.place.length;
  const parts = [
    ...(added ? [m["status.drop.added"]({ count: added })] : []),
    ...(reused.length
      ? [m["status.drop.reused"]({ count: reused.length })]
      : []),
    ...(plan.place.length
      ? [m["status.drop.placing"]({ count: plan.place.length })]
      : []),
    ...(left ? [m["status.drop.noLane"]({ count: left })] : []),
    ...(plan.refused.length
      ? [m["status.drop.refused"]({ files: plan.refused.join(", ") })]
      : []),
  ];
  return parts.join(" — ") || m["status.drop.nothing"]();
}
