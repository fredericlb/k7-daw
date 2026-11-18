// presetOrder.ts — a node's presets in the order the dropdown lists them and its arrows step
// through: each scope where the service put it (workspace first), its names for people.
// One function, so the list and the arrows cannot disagree — ../docs/listOrder.md.

import type { PresetEntry } from "@studio/song-schema/presets";
import { byPeople } from "@studio/web-protocol/tree";

/** The presets in a NEW array: scopes in the order they arrived, names within each for people. */
export function presetsForPeople(
  presets: readonly PresetEntry[],
): PresetEntry[] {
  const rank = new Map<string, number>();
  for (const p of presets) if (!rank.has(p.scope)) rank.set(p.scope, rank.size);
  const at = (p: PresetEntry) => rank.get(p.scope) ?? 0;
  return [...presets].sort((a, b) => at(a) - at(b) || byPeople(a.name, b.name));
}
