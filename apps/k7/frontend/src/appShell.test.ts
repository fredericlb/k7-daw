import { describe, expect, it } from "vitest";
import { midiMissesInDesktop, shellOf } from "./appShell";

describe("the shell a page runs in", () => {
  it("tells a phone's view, the desktop window and a plain browser apart", () => {
    expect(shellOf({ k7Android: {} })).toEqual({
      phone: true,
      desktop: false,
      inApp: true,
    });
    expect(shellOf({ k7Ios: {} })).toEqual({
      phone: true,
      desktop: false,
      inApp: true,
    });
    expect(shellOf({ k7Desktop: { release: "dev" } })).toEqual({
      phone: false,
      desktop: true,
      inApp: true,
    });
    expect(shellOf({})).toEqual({ phone: false, desktop: false, inApp: false });
  });

  it("sends the desktop window without Web MIDI to the browser, and no one else", () => {
    const desktop = { k7Desktop: {} };
    expect(midiMissesInDesktop(desktop, {})).toBe(true);
    expect(midiMissesInDesktop(desktop, { requestMIDIAccess: () => {} })).toBe(
      false,
    );
    expect(midiMissesInDesktop({}, {})).toBe(false);
  });
});
