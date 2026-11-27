// nodeMenu.ts — a lane's gutter menu: its module's other attribute sets, and its freeze, as rows the page can draw.
// A reader would get wrong that the rows are written here: Rust writes them with message keys, and `words` resolves them.

import { type NodeMenuTarget, nodeMenuItems } from "@studio/k7-wasm/menu";
import type { MenuItem } from "./contextMenu";
import { words } from "./menuWords";

export type { NodeMenuTarget };

/** A lane's gutter menu: its module's other attribute sets, and its freeze; this project's data rather than the catalogue. */
export const nodeMenu = (n: NodeMenuTarget): MenuItem[] =>
  nodeMenuItems(n).map((i) => ({
    label: words(i.label),
    description: words(i.description),
    command: i.command,
    complete: i.complete,
    verb: i.verb,
  }));
