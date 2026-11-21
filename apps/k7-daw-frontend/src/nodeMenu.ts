// nodeMenu.ts — a lane's gutter menu: its module's other attribute SETS, and its freeze. This project's
// data rather than the command catalogue, which is why it is not derived like contextMenu.ts's rows.
// See apps/k7-daw-frontend/packages/song/docs/alternatives.md.

import type { MenuItem } from "./contextMenu";
import { m } from "./paraglide/messages.js";

/** What a lane's gutter badge opens ON: the lane's node, as the timeline holds it. */
export interface NodeMenuTarget {
  module?: string;
  alternatives?: string[];
  alternative?: string;
  frozen?: "at" | "inside";
}

/** A lane's gutter menu: its module's other attribute SETS, and its freeze — this project's data rather than the catalogue (apps/k7-daw-frontend/packages/song/docs/alternatives.md). */
export function nodeMenu(n: NodeMenuTarget): MenuItem[] {
  const module = n.module;
  if (!module) return [];
  const names = n.alternatives ?? [];
  const current = n.alternative ?? "1";
  return [
    // A dot marks the one that is on, and the gap on the other rows is two NON-BREAKING spaces, HTML collapsing ordinary ones.
    ...names.map((name) => ({
      label: `${name === current ? "• " : "  "}${name === "1" ? m["node.altOwn"]() : name}`,
      description:
        name === current ? m["node.altOn"]() : m["node.altPlay"]({ name }),
      command: `node alternative select ${module} ${name}`,
      complete: true,
      verb: "node alternative select",
    })),
    {
      label: m["node.altNew"](),
      description: m["node.altNewHelp"](),
      command: `node alternative create ${module} `,
      complete: false,
      verb: "node alternative create",
    },
    {
      label: m["node.altCopy"](),
      description: m["node.altCopyHelp"](),
      command: `node alternative copy ${module} `,
      complete: false,
      verb: "node alternative copy",
    },
    // Freeze: one entry, run on the click, and none at all on a node INSIDE somebody else's render.
    ...(n.frozen === "inside"
      ? []
      : [
          {
            label:
              n.frozen === "at" ? m["node.unfreeze"]() : m["node.freeze"](),
            description:
              n.frozen === "at"
                ? m["node.unfreezeHelp"]()
                : m["node.freezeHelp"](),
            command: `node freeze ${module}`,
            complete: true,
            verb: "node freeze",
          },
        ]),
  ];
}
