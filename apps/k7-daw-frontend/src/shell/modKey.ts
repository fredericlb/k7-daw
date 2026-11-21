// modKey.ts — the platform's name for the key chord that summons the command bar (⌘⇧P on a Mac, Ctrl+Shift+P elsewhere).

const apple = (): boolean =>
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** The summon chord as the person's keyboard spells it. */
export const summonChord = (): string => (apple() ? "⌘⇧P" : "Ctrl+Shift+P");
