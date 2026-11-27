// contextMenu.ts — the right-click entries for a timeline/graph entity: every entry is a command line, derived from the catalogue by Rust.
// A reader would get wrong where the rows come from: `@studio/k7-wasm/menu` derives them with message keys, and `words` gives them the app's language.

import { contextMenuItems, type MenuCtx } from "@studio/k7-wasm/menu";
import type { CommandPrefs } from "./commandPrefs";
import type { ResourceDropEvent } from "./hooks/useTimelineDrop";
import { words } from "./menuWords";
import type { CommandInfo } from "./types";

// A module the ENGINE provides has no file anywhere, and the `@engine/` prefix is exactly what says so.
export { withAmbient } from "@studio/k7-wasm/menu";
// The lane gutter's menu lives in nodeMenu.ts; re-exported for the shell, which imports it from here.
export { type NodeMenuTarget, nodeMenu } from "./nodeMenu";
export type { MenuCtx };

/** A displayable menu item: what the command is CALLED, what it does, and the line it stands for; `complete` says the host RUNS it. */
export interface MenuItem {
  /** The command's NAME, minus the word that names the thing you clicked; the row's text when it has no `sentence`. */
  label: string;
  /** What it DOES, as one sentence in the language being read: put in the STATUS BAR rather than drawn in the row. */
  description: string;
  command: string;
  complete: boolean;
  /** Ask before running it: the catalogue's `confirm:`, for the few that destroy more than Ctrl+Z is trusted with. */
  confirm?: boolean;
  /** The VERB this entry stands for, without the entity it was built for and untranslated: what `commandPrefs` is keyed by. */
  verb: string;
  /** What the ROW reads: the catalogue's `readable` in the language being read, the label then shown beside it as the verb. */
  sentence?: string;
  /** A resource to place: picking the row runs the drop's own steps (a video needs a probe and two follow-ups), not just `command`. */
  place?: ResourceDropEvent;
}

/** The menu for a context; empty means "don't open a menu here". */
export const contextMenu = (
  ctx: MenuCtx,
  commands: CommandInfo[],
  /** The person's own arrangement: only `menu: false` is read here. Absent means nothing hidden. */
  prefs: CommandPrefs = {},
  /** The language the sentences are read in. Names are the CLI's and never translated. */
  locale = "en",
): MenuItem[] =>
  contextMenuItems(ctx, commands, prefs, locale).map((i) => ({
    label: words(i.label),
    description: words(i.description),
    command: i.command,
    complete: i.complete,
    ...(i.confirm ? { confirm: true } : {}),
    verb: i.verb,
    ...(i.sentence ? { sentence: i.sentence } : {}),
  }));
