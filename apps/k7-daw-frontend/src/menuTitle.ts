// menuTitle.ts — the row at the top of a right-click menu naming what it is ON: Rust says the entity and how the name is made, the page's namers make it.
// A reader would get wrong who names a lane or a clip: the host does (a lane by its label, a clip by its shown name), and absent, an id reads as itself.

import { type MenuCtx, menuTargetOf } from "@studio/k7-wasm/menu";
import { noteName } from "./noteNames";
import type { MenuEntity } from "./types";

/** What a menu is on: the kind of thing, and the name it is shown by. */
export interface MenuTarget {
  entity: MenuEntity;
  name: string;
}

/** How ids read as names: the host's lanes and clips; absent, an id reads as itself. */
export interface MenuNamers {
  lane?: (id: string) => string;
  clip?: (ref: string) => string;
}

/** The target of a context's menu, or null for one that names nothing (a chain, a sink). */
export function menuTarget(
  ctx: MenuCtx,
  names: MenuNamers = {},
): MenuTarget | null {
  const t = menuTargetOf(ctx);
  if (!t) return null;
  const lane = names.lane ?? ((id: string) => id);
  const clip = names.clip ?? ((ref: string) => ref);
  const n = t.name;
  const name =
    n.kind === "lane"
      ? lane(n.id)
      : n.kind === "clip"
        ? clip(n.id)
        : n.kind === "note"
          ? `${noteName(n.note)} · ${clip(n.sequence)}`
          : n.text;
  return { entity: t.entity as MenuEntity, name };
}
