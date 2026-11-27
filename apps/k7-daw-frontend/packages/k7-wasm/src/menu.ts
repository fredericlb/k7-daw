// menu.ts — a right-click's rows, tabs and title, the palette's catalogue and vocabularies, and the home page's commands, from Rust.
// A reader would get wrong who writes a row's words: a hand-named row carries a message key in `Text`, which the page resolves in its language.

import { call } from "./abi.ts";
import type {
  ArgSource,
  CommandInfo,
  Danger,
  Focus,
  HomeRow,
  Item,
  LocalCommand,
  MenuCtx,
  MenuTarget,
  NodeMenuTarget,
  PaletteRow,
  Parsed,
  Position,
  Resolved,
  ShellLists,
  TabGroup,
  Text,
  WorkspaceRef,
} from "./types/index.ts";

export { MENU_ENTITIES } from "./consts.generated.ts";

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

/** The right-click's rows for a context, derived from the catalogue; empty means "do not open a menu here". */
export const contextMenuItems = (
  ctx: MenuCtx,
  commands: readonly CommandInfo[],
  prefs: object,
  locale: string,
): Item[] =>
  call<Item[]>({
    op: "menu.context_menu",
    args: { ctx, commands: [...commands], prefs, locale },
  });

/** A module the engine provides has no file anywhere: its context says `ambient`. */
export const withAmbient = (ctx: MenuCtx): MenuCtx =>
  call<MenuCtx>({ op: "menu.with_ambient", args: { ctx } });

/** What a menu is on: the entity, and how its name is made; null for a context that names nothing. */
export const menuTargetOf = (ctx: MenuCtx): MenuTarget | null =>
  call<MenuTarget | null>({ op: "menu.target", args: { ctx } });

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

/** A lane's gutter menu: its module's other attribute sets, and its freeze. */
export const nodeMenuItems = (target: NodeMenuTarget): Item[] =>
  call<Item[]>({ op: "menu.node_menu", args: { target } });

/** The palette's full catalogue: the backend's commands plus the shell's own, each sentence resolved by the locale. */
export const buildCommands = (
  backend: readonly CommandInfo[],
  locals: readonly LocalCommand[],
  locale: string,
): PaletteRow[] =>
  call<PaletteRow[]>({
    op: "menu.build_commands",
    args: { backend: [...backend], locals: [...locals], locale },
  });

/** Which command a canonical line runs: the longest name that prefixes it. */
export const commandNameOf = (
  line: string,
  commands: readonly string[],
): string | null =>
  call<{ canonical: string | null }>({
    op: "menu.command_name_of",
    args: { line, commands: [...commands] },
  }).canonical;

/** Where a field's whole vocabulary comes from: a list known here, or the backend's completion of this line. */
export const argSource = (
  canonical: string,
  prior: readonly string[],
  lists: ShellLists,
  entryArgIsFile: boolean,
): ArgSource =>
  call<ArgSource>({
    op: "menu.arg_source",
    args: { canonical, prior: [...prior], lists, entryArgIsFile },
  });

// ---- the home page ----------------------------------------------------------

/** The home page's catalogue, each sentence resolved by the locale. */
export const homeCatalogue = (locale: string): HomeRow[] =>
  call<HomeRow[]>({ op: "menu.home_catalogue", args: { locale } });

/** Which command a home line is: nothing, none of this screen's, or a command and the rest of the line. */
export const homeParse = (line: string): Parsed =>
  call<Parsed>({ op: "menu.home_parse", args: { line } });

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

/** The message refusing a project no workspace holds, naming the ones there are. */
export const homeGripe = (workspace: WorkspaceRef, name: string): Text =>
  call<Text>({ op: "menu.home_gripe", args: { workspace, name } });

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
