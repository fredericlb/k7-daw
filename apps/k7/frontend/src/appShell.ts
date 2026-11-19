// appShell.ts — which app, if any, holds this page: a phone's view (`k7Android`, `k7Ios`) or the desktop app's
// window (`k7Desktop`, from the host's init script). A plain browser has none. See docs/localApp.md.

type Marked = { k7Android?: unknown; k7Ios?: unknown; k7Desktop?: unknown };

export interface Shell {
  phone: boolean;
  desktop: boolean;
  /** Any app's view: its files are on the machine, so no service worker caches them again. */
  inApp: boolean;
}

/** The shell `win` runs in, read off the markers the hosts set before the page's own code. */
export function shellOf(win: object): Shell {
  const w = win as Marked;
  const phone = !!(w.k7Android ?? w.k7Ios);
  const desktop = !!w.k7Desktop;
  return { phone, desktop, inApp: phone || desktop };
}

export const shell: Shell = shellOf(
  typeof window === "undefined" ? {} : window,
);

/** Whether the window lacks Web MIDI where the desktop app's webview is known to (macOS, Linux): the browser has it. */
export function midiMissesInDesktop(win: object, nav: object): boolean {
  return (
    shellOf(win).desktop &&
    !("requestMIDIAccess" in nav && (nav as Navigator).requestMIDIAccess)
  );
}
