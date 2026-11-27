// clipOps.ts — running a library CHAIN over one clip: the vocabulary the waveform panel's menu and the editor's Process button share.
// A reader would get wrong where the line is spelled: Rust spells it (`@studio/k7-wasm/edit`), so a second surface cannot differ.

import { filterOps, processLine } from "@studio/k7-wasm/edit";
import { byPeoplePath } from "@studio/web-protocol/tree";

export { filterOps, processLine };

/** Every library chain, as `clip process` names it, sorted for people: the browser's collator is the order. */
export function chainOps(
  chains: { ref: string }[] | null | undefined,
): string[] {
  return (chains ?? []).map((c) => c.ref).sort(byPeoplePath);
}
