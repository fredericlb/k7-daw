// rackBinds.ts — how a rack pad's `bind` is spelled: `@rack:BD` plays a sound, `@rack+` and `@rack+<note>` add one.
// A reader would get wrong that a bare `@rack+` is an add tap too: it picks the note itself, so its note is null.

import { RACK_ADD_BIND, RACK_BIND } from "./consts.generated.ts";

export { RACK_ADD_BIND, RACK_BIND };

/** An add tap on ONE note: an empty cell of a rack's octave grid, `@rack+<note>`. */
export const rackAddBindAt = (note: number): string =>
  `${RACK_ADD_BIND}${note}`;

/** Whether a bind is an add tap, and the note it adds on; null for a bind that is not one. */
export function rackAddBind(
  bind: string | undefined,
): { note: number | null } | null {
  if (!bind?.startsWith(RACK_ADD_BIND)) return null;
  const rest = bind.slice(RACK_ADD_BIND.length);
  if (!rest) return { note: null };
  const note = Number(rest);
  return Number.isInteger(note) && note >= 0 && note <= 127 ? { note } : null;
}

/** The symbol a rack-pad bind names, or null for any other bind. */
export const rackBindSymbol = (bind: string | undefined): string | null =>
  bind?.startsWith(RACK_BIND) ? bind.slice(RACK_BIND.length) : null;
