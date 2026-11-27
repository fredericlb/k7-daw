// CommandMenuPicker.tsx — the "where does this show up" cell of the Commandes settings page:
// which right-click menus may offer one command. The catalogue names them (commands.yaml's
// `menu:`), this is the person's own list over it — several at once, since one verb belongs on
// two menus. See ../../docs/contextMenu.md.

import { menuChoice } from "@studio/k7-wasm/ui";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { cn } from "@studio/ui/lib/utils";
import type { CommandPref } from "../commandPrefs";
import { m } from "../paraglide/messages.js";
import { MENU_ENTITIES, type MenuEntity } from "../types";
import type { PaletteCommand } from "./CommandBar";

/** What each entity is CALLED here — the word the catalogue uses is the id, and this is the row you read. */
const ENTITY_LABEL: Record<MenuEntity, () => string> = {
  track: () => m["menuEntity.track"](),
  group: () => m["menuEntity.group"](),
  node: () => m["menuEntity.node"](),
  event: () => m["menuEntity.event"](),
  param: () => m["menuEntity.param"](),
  curve: () => m["menuEntity.curve"](),
  ruler: () => m["menuEntity.ruler"](),
  marker: () => m["menuEntity.marker"](),
  timeline: () => m["menuEntity.timeline"](),
  assembly: () => m["menuEntity.assembly"](),
  warp: () => m["menuEntity.warp"](),
  shot: () => m["menuEntity.shot"](),
  picture: () => m["menuEntity.picture"](),
  note: () => m["menuEntity.note"](),
  tap: () => m["menuEntity.tap"](),
  rack: () => m["menuEntity.rack"](),
};

/** WHERE a right-click offers this command: the catalogue's own list until someone picks another.
 *  Several entities at once, since one verb belongs on two menus — `node param` on a bloc and on a réglage. */
export function MenuEntityPicker({
  c,
  pref,
  onMenu,
}: {
  c: PaletteCommand;
  pref: CommandPref | undefined;
  onMenu: (
    canonical: string,
    catalogue: readonly MenuEntity[] | undefined,
    next: readonly MenuEntity[],
  ) => void;
}) {
  // null is "partout où ça colle" — the derivation, which is what an untouched command with no `menu:` does.
  const chosen = menuChoice(pref, c.menu);
  const on = chosen ?? MENU_ENTITIES;
  const summary = !chosen
    ? m["cmdMenu.everywhere"]()
    : chosen.length === 0
      ? m["cmdMenu.never"]()
      : chosen.length === 1
        ? chosen[0]
        : m["cmdMenu.types"]({ n: chosen.length });
  const toggle = (e: MenuEntity) =>
    onMenu(
      c.canonical,
      c.menu,
      on.includes(e) ? on.filter((x) => x !== e) : [...on, e],
    );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          title={m["cmdMenu.buttonHint"]()}
          className={cn(
            "h-9 w-24 shrink-0 cursor-pointer truncate rounded-xl border border-(--k7-hairline-strong) px-2 type-secondary",
            chosen ? "text-fg-1" : "text-fg-2",
          )}
        >
          {summary}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="p-1">
        <DropdownMenuLabel>{m["cmdMenu.label"]()}</DropdownMenuLabel>
        {/* Checkbox items: arrows reach them, and a pick leaves the menu open so several can be chosen. */}
        {MENU_ENTITIES.map((e) => (
          <DropdownMenuCheckboxItem
            key={e}
            checked={on.includes(e)}
            onCheckedChange={() => toggle(e)}
          >
            <span className="flex-1">{ENTITY_LABEL[e]()}</span>
            <code className="type-badge text-fg-2">{e}</code>
          </DropdownMenuCheckboxItem>
        ))}
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={!chosen}
          onSelect={() => onMenu(c.canonical, c.menu, c.menu ?? MENU_ENTITIES)}
          className="type-secondary text-fg-2"
        >
          {m["cmdMenu.giveBack"]({
            what: c.menu ? c.menu.join(", ") : m["cmdMenu.everywhere"](),
          })}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
