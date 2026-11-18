// locked.ts — what a local command says and does when the workspace is read-only for this device.

import { refusedWrite } from "@studio/project-shell/fileApi";
import { m } from "../../paraglide/messages.js";
import type { LocalCtx } from "./context";

/** Refuses a local command that writes while the workspace is locked: says why and returns true. */
export function refuseWhileLocked(ctx: LocalCtx, line: string): boolean {
  if (!ctx.locked) return false;
  ctx.setStatus(
    refusedWrite({ kind: "command", line }) ?? m["git.lockedByMerge"](),
  );
  return true;
}

/** The sentence that refuses an import's writes, or null: the Worker writes through routes the gate never sees. */
export const importRefusal = (): string | null =>
  refusedWrite({ kind: "command", line: "project import" });
