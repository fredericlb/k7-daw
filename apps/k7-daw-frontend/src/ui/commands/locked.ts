// locked.ts — what a local command says and does when the workspace is read-only for this device.

import { m } from "../../paraglide/messages.js";
import type { LocalCtx } from "./context";

/** Refuses a local command that writes while the workspace is locked: says why and returns true. */
export function refuseWhileLocked(ctx: LocalCtx, _line: string): boolean {
  if (!ctx.locked) return false;
  ctx.setStatus(m["git.lockedByMerge"]());
  return true;
}
