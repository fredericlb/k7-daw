// midiMessage.ts — what the status line says when Web MIDI is not there: in the desktop app's window, where macOS and
// Linux webviews have none, it names the way out (the browser) instead of blaming a refusal.

import { midiMissesInDesktop } from "./appShell";
import { m } from "./paraglide/messages.js";

export const midiUnavailableText = (): string =>
  midiMissesInDesktop(window, navigator)
    ? m["midi.unavailableDesktop"]()
    : m["midi.unavailable"]();
