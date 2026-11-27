// ContextMenu.tsx — the right-click popup shared by the timeline and the chains graph: a title
// naming what it is on, then each command as its sentence, its verb and its key, the destructive
// ones last and red; on a pick it PRE-WRITES the line into the bar (see contextMenu.ts). One
// instance lives in App; both surfaces open it through openContextMenu. Dismisses on Escape, an
// outside click, a scroll, losing focus or its close button. Rows are split into TABS (menuTabs.ts).

import {
  PointMenu,
  PointMenuItem,
  PointMenuSeparator,
} from "@studio/ui/point-menu";
import { useEffect, useMemo, useState } from "react";
import { type CommandPrefs, labelIn } from "../commandPrefs";
import type { MenuItem } from "../contextMenu";
import { useLocale } from "../i18n";
import { shortcutsByCommand } from "../keys";
import { useIcons } from "../lucideIcons";
import { dangerLast, menuTabs, moveFocus } from "../menuTabs";
import type { MenuTarget } from "../menuTitle";
import { m } from "../paraglide/messages.js";
import { ContextMenuHeader } from "./ContextMenuHeader";

export function ContextMenu({
  items,
  prefs,
  target,
  x,
  y,
  onPick,
  onHint,
  onClose,
}: {
  items: MenuItem[];
  /** This person's layer over the catalogue, applied HERE rather than in `contextMenu.ts`, which stays a pure function. */
  prefs: CommandPrefs;
  /** What the menu is on, for its title row; absent, the row holds only the touch close button. */
  target?: MenuTarget | null;
  x: number; // viewport coords of the click
  y: number;
  // The WHOLE item, not its line: whether every argument is filled decides whether the host runs it or hands it to the bar.
  onPick: (item: MenuItem) => void;
  /** What the entry under the pointer DOES, for the host's status bar — must be STABLE, being called from an unmount cleanup. */
  onHint?: (description: string) => void;
  onClose: () => void;
}) {
  // Fetched only when some row carries a person's icon; null until it lands, so the first menu draws labels alone.
  const icons = useIcons(items.some((it) => !!prefs[it.verb]?.icon));
  // A person's name for a command is per LANGUAGE, so picking one takes a locale — read here, exactly as `useIcons` is.
  const { locale } = useLocale();
  // The key each command answers to, read off the person's own table so a row never advertises a key nothing binds.
  const keys = useMemo(() => shortcutsByCommand(prefs), [prefs]);
  // The rows by tab, and the tab on show: the first, which is the thing you clicked, on every new menu.
  const tabs = useMemo(() => menuTabs(items), [items]);
  const [tab, setTab] = useState(0);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new list of rows is a new menu, opened on its first tab.
  useEffect(() => setTab(0), [items]);
  const on = Math.min(tab, tabs.length - 1);
  const { safe, danger } = dangerLast(tabs[on]?.items ?? []);
  // Another tab is another height and another first row: the kit re-clamps and refocuses on a new key.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `items` and `on` ARE the trigger.
  const refresh = useMemo(() => ({}), [items, on]);
  // Left and right walk the tabs, as they do in any tab strip.
  const walkTabs = (e: KeyboardEvent) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return false;
    setTab((t) => {
      const n = tabs.length;
      return n ? (t + (e.key === "ArrowLeft" ? n - 1 : 1)) % n : 0;
    });
    return true;
  };

  // The menu going takes its line with it, or the last thing hovered would read as something the app just did.
  useEffect(() => () => onHint?.(""), [onHint]);

  if (!items.length) return null;

  const row = (it: MenuItem, destructive: boolean) => {
    const pref = prefs[it.verb];
    const Icon = pref?.icon ? icons?.[pref.icon] : undefined;
    // The person's name in THIS language, else the catalogue's sentence, else the entry's name (docs/contextMenu.md).
    const text = labelIn(pref?.label, locale).trim() || it.sentence || it.label;
    return (
      <PointMenuItem
        key={it.command}
        danger={destructive}
        icon={Icon}
        // The icon gutter is held whether or not this row has one, so the texts line up.
        gutter
        shortcut={keys[it.verb]}
        title={
          it.complete
            ? m["menu.runs"]({ command: it.command })
            : m["menu.fills"]({ command: it.command })
        }
        onSelect={() => onPick(it)}
        // The status line follows the pointer, and the KEYBOARD too — walking the menu is a way of reading it.
        onPointerEnter={() => onHint?.(it.description)}
        onFocus={() => onHint?.(it.description)}
      >
        <span className="min-w-0 flex-1 truncate">{text}</span>
        {/* The verb, muted, whenever the row reads as something else: it is what you would type. */}
        {text !== it.label && (
          <span className="max-w-[8rem] shrink-0 truncate font-mono text-[11px] text-fg-2">
            {it.label}
          </span>
        )}
        {/* Whether picking this RUNS it or only fills the bar with it — an action against a question, worth a character per row. */}
        {!it.complete && <span className="shrink-0 text-fg-2">…</span>}
      </PointMenuItem>
    );
  };

  return (
    <PointMenu
      x={x}
      y={y}
      onClose={onClose}
      label={target?.name}
      refreshKey={refresh}
      moveFocus={moveFocus}
      onKey={walkTabs}
      // One handler for the whole list: leaving the menu by the gap between two rows should clear the line, and only the container sees that.
      onPointerLeave={() => onHint?.("")}
    >
      <ContextMenuHeader
        target={target}
        tabs={tabs}
        on={on}
        onTab={setTab}
        onClose={onClose}
      />
      {safe.map((it) => row(it, false))}
      {safe.length > 0 && danger.length > 0 && <PointMenuSeparator />}
      {danger.map((it) => row(it, true))}
    </PointMenu>
  );
}
