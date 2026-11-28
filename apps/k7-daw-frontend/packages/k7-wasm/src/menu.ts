// menu.ts — a right-click's rows, tabs and title, the palette's catalogue and vocabularies, and the home page's commands, from Rust.
// A reader would get wrong who writes a row's words: a hand-named row carries a message key in `Text`, which the page resolves in its language.

import { call } from "./abi.ts";
import type {
  Danger,
  Focus,
  Position,
  Resolved,
  TabGroup,
  WorkspaceRef,
} from "./types/index.ts";

export { MENU_ENTITIES } from "./consts.generated.ts";
export * from "./ops/menu.ts";

export type {
  ArgClass,
  ArgProvider,
  ArgSource,
  CommandArg,
  CommandInfo,
  HomeRow,
  Item,
  LocalCommand,
  MenuCtx,
  MenuEntity,
  MenuTarget,
  NameSpec,
  NodeMenuTarget,
  PaletteRow,
  Parsed,
  Resolved,
  ShellLists,
  TabGroup,
  Text,
  WorkspaceRef,
} from "./types/index.ts";

type Row = { verb: string; confirm?: boolean };
const rowsOf = (items: readonly Row[]) =>
  items.map((i) => ({
    verb: i.verb,
    ...(i.confirm ? { confirm: true } : {}),
  }));

/** The rows grouped into tabs, in the order the menu offers them: the shell's own `ui` tab last. */
export function menuTabs<T extends Row>(
  items: readonly T[],
): { tab: string; items: T[] }[] {
  return call<TabGroup[]>({
    op: "menu.tabs",
    args: { rows: rowsOf(items) },
  }).map((g) => ({ tab: g.tab, items: g.indices.map((i) => items[i] as T) }));
}

/** A tab's rows split in two, each half in its own order: the destructive half is drawn last. */
export function dangerLast<T extends Row>(
  items: readonly T[],
): { safe: T[]; danger: T[] } {
  const d = call<Danger>({
    op: "menu.danger_split",
    args: { rows: rowsOf(items) },
  });
  return {
    safe: d.safe.map((i) => items[i] as T),
    danger: d.danger.map((i) => items[i] as T),
  };
}

/** Where Up, Down, Home and End move the focus among `n` rows from `at` (-1 when none has it), or null for another key. */
export const moveFocus = (key: string, at: number, n: number): number | null =>
  call<Focus>({ op: "menu.move_focus", args: { key, at, n } }).at;

/** Which command a canonical line runs: the longest name that prefixes it. */
export const commandNameOf = (
  line: string,
  commands: readonly string[],
): string | null =>
  call<{ canonical: string | null }>({
    op: "menu.command_name_of",
    args: { line, commands: [...commands] },
  }).canonical;

// ---- the home page ----------------------------------------------------------

type HomeCtx = {
  workspaces: WorkspaceRef[];
  openWs?: string | null;
};

/** Which workspace a `project ...` line acts in, by position, or the message refusing it. */
export const homeResolve = (ctx: HomeCtx): Resolved =>
  call<Resolved>({
    op: "menu.home_resolve",
    args: { ctx: { ...ctx, openWs: ctx.openWs ?? null } },
  });

/** A project of that workspace, by position: named by its path, its folder or its stem. */
export const homeFindProject = (
  workspace: WorkspaceRef,
  name: string,
): number | null =>
  call<Position>({ op: "menu.home_find_project", args: { workspace, name } })
    .index;

/** One param's whole vocabulary for the home page's open field. */
export const homeArgValues = (
  canonical: string,
  argIndex: number,
  ctx: HomeCtx,
): string[] =>
  call<string[]>({
    op: "menu.home_arg_values",
    args: { canonical, argIndex, ctx: { ...ctx, openWs: ctx.openWs ?? null } },
  });
