// The undo/redo flash stacks the steps of one kind, starts over on the other, and goes on its own.

import { afterEach, describe, expect, it, vi } from "vitest";
import { FLASH_MS, flashSnapshot, flashStep } from "./undoFlashStore";

afterEach(() => vi.useRealTimers());

describe("the undo flash", () => {
  it("stacks undos, restarts on a redo, and clears after the delay", () => {
    vi.useFakeTimers();
    flashStep("undo", "Undone: tempo");
    flashStep("undo", "Undone: a clip");
    expect(flashSnapshot()).toMatchObject({
      kind: "undo",
      messages: ["Undone: tempo", "Undone: a clip"],
    });
    flashStep("redo", "Redone: a clip");
    expect(flashSnapshot()).toMatchObject({
      kind: "redo",
      messages: ["Redone: a clip"],
    });
    vi.advanceTimersByTime(FLASH_MS + 1);
    expect(flashSnapshot()).toBeNull();
  });
});
