import { afterEach, describe, expect, it, vi } from "vitest";
import { copyText } from "./copyText";

/** A textarea as far as the copy touches it. */
const fakeArea = () => ({
  value: "",
  readOnly: false,
  style: {} as Record<string, string>,
  selected: [] as number[],
  focus: vi.fn(),
  select: vi.fn(),
  setSelectionRange(a: number, b: number) {
    this.selected = [a, b];
  },
  remove: vi.fn(),
});

const page = (execCommand: () => boolean, made = fakeArea()) => {
  vi.stubGlobal("document", {
    createElement: () => made,
    body: { appendChild: vi.fn() },
    execCommand,
  });
  return made;
};

afterEach(() => vi.unstubAllGlobals());

describe("copyText", () => {
  it("uses the clipboard API when the page has it", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    expect(await copyText("ssh-ed25519 AAA")).toBe(true);
    expect(writeText).toHaveBeenCalledWith("ssh-ed25519 AAA");
  });

  it("selects the shown field and runs execCommand when the API refuses", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error("denied")) },
    });
    const exec = vi.fn().mockReturnValue(true);
    const area = page(exec);
    expect(await copyText("ssh-ed25519 AAA", area as never)).toBe(true);
    expect(exec).toHaveBeenCalledWith("copy");
    expect(area.selected).toEqual([0, 15]);
    expect(area.remove).not.toHaveBeenCalled();
  });

  it("makes a hidden field when none is shown, and removes it", async () => {
    vi.stubGlobal("navigator", {});
    const made = page(() => true);
    expect(await copyText("abc")).toBe(true);
    expect(made.value).toBe("abc");
    expect(made.remove).toHaveBeenCalled();
  });

  it("says false when neither way works", async () => {
    vi.stubGlobal("navigator", {});
    page(() => false);
    expect(await copyText("x")).toBe(false);
  });
});
