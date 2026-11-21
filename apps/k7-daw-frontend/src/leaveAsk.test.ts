import { beforeEach, describe, expect, it, vi } from "vitest";

const ask = vi.fn();
let unsaved = false;
vi.mock("@studio/project-shell/ask", () => ({
  askConfirm: (q: unknown) => ask(q),
}));
vi.mock("@studio/ui-editor/unsaved", () => ({
  hasUnsavedWork: () => unsaved,
  unsavedKind: () => "edits",
}));
vi.mock("./paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_, k) => () => String(k) }),
}));

const { mayLeave } = await import("./leaveAsk");

describe("leaving a project", () => {
  beforeEach(() => {
    ask.mockReset();
    unsaved = false;
  });

  it("asks nothing when everything is saved", async () => {
    expect(await mayLeave()).toBe(true);
    expect(ask).not.toHaveBeenCalled();
  });

  it("asks through the app's own question, whose answer is the answer", async () => {
    unsaved = true;
    ask.mockResolvedValueOnce(false);
    expect(await mayLeave()).toBe(false);
    ask.mockResolvedValueOnce(true);
    expect(await mayLeave()).toBe(true);
    expect(ask).toHaveBeenCalledTimes(2);
  });
});
