// askFootswitch.ts — the question a slot's Footswitch button asks: the MIDI CC whose press bypasses its chain, empty
// for none. A press toggles; a latching pedal is set to momentary. See ../../docs/slots.md.

import { askText } from "@studio/project-shell/ask";
import { m } from "../paraglide/messages.js";

/** The CC chosen, null for none, undefined when the question was dismissed. */
export async function askFootswitch(
  name: string,
  current: number | null | undefined,
): Promise<number | null | undefined> {
  const asked = await askText({
    title: m["slots.footswitchTitle"]({ name }),
    body: m["slots.footswitchBody"](),
    initial: current == null ? "" : String(current),
    placeholder: "64",
    validate: (v) =>
      v.trim() === "" || /^(\d|\d\d|1[01]\d|12[0-7])$/.test(v.trim())
        ? ""
        : m["slots.footswitchBad"](),
  });
  if (asked === null) return undefined;
  return asked.trim() === "" ? null : Number(asked.trim());
}
