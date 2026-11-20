// The local writers a merge's gate cannot see: each is refused by the command itself.

import { setWriteGate } from "@studio/project-shell/fileApi";
import { afterEach, describe, expect, it, vi } from "vitest";
import { runLocal } from "../ui/commands/dispatch";
import type { LocalCtx } from "../ui/commands/index";
import { importRefusal } from "../ui/commands/locked";
import { mergeGate } from "./gitLock";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

afterEach(() => setWriteGate(null));

function ctxOf(over: Partial<LocalCtx>) {
  const status: string[] = [];
  const ctx = {
    setStatus: (s: string) => status.push(s),
    editorRef: { current: { entries: () => ["x.als"] } },
    ...over,
  } as unknown as LocalCtx;
  return { ctx, status };
}

describe("a locked workspace's local writers", () => {
  it("`project import` asks nothing and says it is locked", () => {
    const askImport = vi.fn();
    const { ctx, status } = ctxOf({
      locked: true,
      imp: { askImport } as unknown as LocalCtx["imp"],
    });
    runLocal(ctx, "project import x.als");
    expect(askImport).not.toHaveBeenCalled();
    expect(status.length).toBe(1);
  });
  it("`transport record` does not arm the engine", () => {
    const setRecordEnabled = vi.fn();
    const { ctx, status } = ctxOf({
      locked: true,
      engineRef: { current: { setRecordEnabled } } as never,
      store: { getSnapshot: () => ({ recordEnabled: false }) } as never,
    });
    runLocal(ctx, "transport record");
    expect(setRecordEnabled).not.toHaveBeenCalled();
    expect(status.length).toBe(1);
  });
  it("a bare play falls back to the server while the engine is busy", () => {
    const { ctx } = ctxOf({
      locked: true,
      connected: false,
      recordWithoutArmRef: { current: false },
    });
    expect(runLocal(ctx, "transport play")).toBe(false);
  });
  it("the import dialog's own write is refused by the gate", () => {
    expect(importRefusal()).toBe(null);
    setWriteGate(mergeGate);
    expect(importRefusal()).toBe("git.lockedByMerge");
  });
});
