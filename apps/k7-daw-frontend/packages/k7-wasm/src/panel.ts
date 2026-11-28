// panel.ts — the module builder's panel rules from Rust: groups and cells, the two layouts, tabs, the clipboard, retyping, the auto layout.
// A reader would get wrong that a panel here is loose data mid-edit, never validated: each call is one JSON round trip and answers a new value, none edits in place.

import type { WebModuleUI, WebWidget } from "@studio/graph-model";
import { call } from "./abi.ts";
import {
  BINDABLE_WIDGET_TYPES,
  GENERAL_PAGE,
  GROUP_COLOR,
  KNOB_SPAN,
  LAYOUT_MAX,
  RANGED_WIDGET_TYPES,
} from "./consts.generated.ts";
import type {
  AiContext,
  AiEvent,
  AiRequest,
  AiResult,
  AutoParam,
  Cell,
  Checked,
  Clip,
  Page,
  PanelRead,
  ParamRowOut,
  Pasted,
  Retyped,
  RowSources,
  Tabbed,
} from "./types/index.ts";

export type {
  AiContext,
  AiEvent,
  AiRequest,
  AiResult,
  AutoParam,
  Cell,
  Checked,
  Clip,
  Page,
  ParamRowOut,
  RowSources,
};
export { GENERAL_PAGE, GROUP_COLOR, KNOB_SPAN, LAYOUT_MAX };

type Widgets = Record<string, WebWidget>;

/** Which widget map the grid edits: the widgets every tab shows (`null`), or one tab by position. */
export type PageRef = number | null;

// ---- groups and the grid ------------------------------------------------

/** Whether two widgets COLLIDE: they overlap, unless one is a group wholly framing the other (never two groups). */
export const clash = (a: WebWidget, b: WebWidget): boolean =>
  call<boolean>({ op: "panel.clash", args: { a, b } });

/** The ids of the widgets `group` frames: every non-group wholly inside its box. */
export const framedBy = (widgets: Widgets, group: WebWidget): string[] =>
  call<string[]>({ op: "panel.framed_by", args: { widgets, group } });

/** The id of the group that frames widget `id`, or null; at most one, since groups never overlap. */
export const groupOf = (widgets: Widgets, id: string): string | null =>
  call<string | null>({ op: "panel.group_of", args: { widgets, id } });

/** Every way a panel's two shapes break the rules: sizes, the twin's own geometry, and controls one shape lacks. */
export const twinIssues = (ui: WebModuleUI): string[] =>
  call<string[]>({ op: "panel.twin_issues", args: { ui } });

/** A first VERTICAL layout from the panel: every control at its size, packed into 4 columns in reading order. */
export const reflowNarrow = (
  ui: WebModuleUI,
): NonNullable<WebModuleUI["vertical"]> =>
  call<NonNullable<WebModuleUI["vertical"]>>({
    op: "panel.reflow_narrow",
    args: { ui },
  });

/** The vertical layout's tabs brought in line with the panel's. */
export const alignTabs = (ui: WebModuleUI): WebModuleUI =>
  call<WebModuleUI>({ op: "panel.align_tabs", args: { ui } });

/** A widget's span: a missing or short one is one cell. */
export const span = (n?: number): number => Math.max(1, n ?? 1);

/** The first cell in `box` no widget covers, row-major; a group frames cells rather than filling them. */
export const firstFree = (
  widgets: Widgets,
  box: { col: number; row: number; cols: number; rows: number },
): Cell =>
  call<Cell>({
    op: "panel.first_free",
    args: { widgets, ...box },
  });

/** A resize handle, named by the corner it sits on. */
export type Corner = "nw" | "ne" | "sw" | "se";
export const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];

/** The cell a corner's resize keeps still: the widget's OPPOSITE corner cell. */
export const cornerAnchor = (w: WebWidget, corner: Corner): Cell =>
  call<Cell>({ op: "panel.corner_anchor", args: { w, corner } });

/** The box spanning two cells, whichever way round they are. */
export function boxBetween(
  a: Cell,
  b: Cell,
): { col: number; row: number; colSpan: number; rowSpan: number } {
  const [col, row, colSpan, rowSpan] = call<number[]>({
    op: "panel.box_between",
    args: { a, b },
  });
  return { col, row, colSpan, rowSpan };
}

/** The widgets in paint order: a group first, under what it frames. */
export const groupsFirst = (ws: Widgets): [string, WebWidget][] =>
  call<string[]>({ op: "panel.groups_first", args: { widgets: ws } }).map(
    (id) => [id, ws[id]],
  );

/** A stable, unused id across the WHOLE panel, every tab's included. */
export const nextId = (taken: Set<string>): string =>
  call<string>({ op: "panel.next_id", args: { taken: [...taken] } });

// ---- tabs ----------------------------------------------------------------

/** `at` checked against the document: nothing chosen, or a tab that has gone, sends the grid to the default. */
export const pageIn = (ui: WebModuleUI, at: PageRef | undefined): PageRef =>
  call<PageRef>({
    op: "panel.page_in",
    args: at === undefined ? { ui } : { ui, at },
  });

/** The map `at` names: the panel's shared widgets, or one tab's. */
export const widgetsAt = (ui: WebModuleUI, at: PageRef): Widgets =>
  call<Widgets>({ op: "panel.widgets_at", args: { ui, at } });

/** `ui` with the map at `at` replaced; the other tabs are untouched. */
export const writeWidgetsAt = (
  ui: WebModuleUI,
  at: PageRef,
  next: Widgets,
): WebModuleUI =>
  call<WebModuleUI>({ op: "panel.write_widgets_at", args: { ui, at, next } });

/** Every widget id on the panel, every tab's included. */
export const panelIds = (ui: WebModuleUI): Set<string> =>
  new Set(call<string[]>({ op: "panel.panel_ids", args: { ui } }));

/** A tab added at the end; the FIRST one added makes two. */
export const addTab = (ui: WebModuleUI): Tabbed =>
  call<Tabbed>({ op: "panel.add_tab", args: { ui } });

/** Tab `at` removed with its widgets; down to one, the list folds away. */
export const removeTab = (ui: WebModuleUI, at: number): WebModuleUI =>
  call<WebModuleUI>({ op: "panel.remove_tab", args: { ui, at } });

/** Tab `at` renamed, or `ui` unchanged when the name is empty or another tab's. */
export const renameTab = (
  ui: WebModuleUI,
  at: number,
  name: string,
): WebModuleUI =>
  call<WebModuleUI>({ op: "panel.rename_tab", args: { ui, at, name } });

/** Tab `at` moved one place up (-1) or down (+1). */
export const moveTab = (ui: WebModuleUI, at: number, by: -1 | 1): Tabbed =>
  call<Tabbed>({ op: "panel.move_tab", args: { ui, at, by } });

// ---- the clipboard -------------------------------------------------------

/** The ids a cut or a copy of `id` takes: a group brings what it frames, first in the list. */
export const clipIds = (widgets: Widgets, id: string): string[] =>
  call<string[]>({ op: "panel.clip_ids", args: { widgets, id } });

/** A copy of `id` (and, for a group, its members), moved so the block starts at cell 0,0. */
export const copyWidgets = (widgets: Widgets, id: string): Clip | null =>
  call<Clip | null>({ op: "panel.copy_widgets", args: { widgets, id } });

/** `widgets` with `clip` dropped at `at`, pulled back inside the grid; `first` is the block's head, to select. */
export const pasteWidgets = (
  widgets: Widgets,
  clip: Clip,
  at: Cell,
  grid: { cols: number; rows: number },
  taken: Set<string>,
): { widgets: Widgets; first: string | null } =>
  call<Pasted>({
    op: "panel.paste_widgets",
    args: { widgets, clip, at, grid, taken: [...taken] },
  });

// ---- kinds ----------------------------------------------------------------

const BINDABLE = new Set<string>(BINDABLE_WIDGET_TYPES);
const RANGED = new Set<string>(RANGED_WIDGET_TYPES);

/** Which widget kinds carry a `bind`, and so the only ones a dragged parameter can bounce off. */
export const bindable = (w: Pick<WebWidget, "type">): boolean =>
  BINDABLE.has(w.type);

/** Which carry a RANGE: not a dropdown, whose options are the set. */
export const ranged = (w: Pick<WebWidget, "type">): boolean =>
  RANGED.has(w.type);

/** The type plus explicit undefineds for the fields the new type lacks; the caller merges, and a writer drops undefined. */
export function retype(
  w: WebWidget,
  type: WebWidget["type"],
): Partial<WebWidget> {
  const { set, clear } = call<Retyped>({
    op: "panel.retype",
    args: { w, type },
  });
  const out: Partial<WebWidget> = { type, ...set };
  for (const key of clear) (out as Record<string, unknown>)[key] = undefined;
  return out;
}

// ---- auto layout ----------------------------------------------------------

/** Which family a parameter's NAME suggests. */
export const familyOf = (name: string): string =>
  call<string>({ op: "panel.family_of", args: { name } });

/** Whether a parameter is plumbing the proposal skips. */
export const isPlumbing = (name: string): boolean =>
  call<boolean>({ op: "panel.is_plumbing", args: { name } });

/** Whether a parameter is the module's OWN blend: the test is the FAMILY, not the word. */
export const isBlend = (name: string): boolean =>
  call<boolean>({ op: "panel.is_blend", args: { name } });

/** The pages that got something, never a `drop` one, in the table's own order. */
export const groupParams = (params: readonly AutoParam[]): Page[] =>
  call<Page[]>({ op: "panel.group_params", args: { params: [...params] } });

/** A whole panel proposed from a parameter list, in both its shapes. */
export const autoOrganize = (params: readonly AutoParam[]): WebModuleUI =>
  call<WebModuleUI>({
    op: "panel.auto_organize",
    args: { params: [...params] },
  });

// ---- a panel in its file ---------------------------------------------------

/** The `ui(…)` block of a module file (or, with `file`, the panel a whole `ui.k7x` declares), and whether a stated panel is unreadable. */
export const readUi = (text: string, file = false): PanelRead =>
  call<PanelRead>({ op: "panel.read_ui", args: { text, file } });

/** The text rewritten from the model with the fewest edits, a widget a line; a buffer that does not parse comes back unchanged. */
export const writeUi = (text: string, ui: WebModuleUI, file = false): string =>
  call<string>({ op: "panel.write_ui", args: { text, ui, file } });

/** The `params(…)` block as name to value as TEXT, never numbers: `1.0` stays `1.0`. */
export const readParams = (content: string): Record<string, string> =>
  call<Record<string, string>>({ op: "panel.read_params", args: { content } });

/** The same values as numbers; anything that is not one is left out. */
export const paramValues = (content: string): Record<string, number> =>
  call<Record<string, number>>({ op: "panel.param_values", args: { content } });

/** Every addressable param unioned from all the sources, in the order met: the page sorts them with its own collator. */
export const paramRows = (
  sources: Partial<RowSources> & Pick<RowSources, "path">,
): ParamRowOut[] =>
  call<ParamRowOut[]>({ op: "panel.param_rows", args: { sources } });

/** The footer an effect or an instrument gets, by its folder. */
export const stageFor = (path: string, io: string | null): string[] =>
  call<string[]>({
    op: "panel.stage_for",
    args: io === null ? { path } : { path, io },
  });

/** Every issue a library panel has: its text, the text git holds for it, and the screens its widgets.js draws. */
export const checkLayout = (
  text: string,
  committed: string | null,
  drawn: readonly string[],
): string[] =>
  call<string[]>({
    op: "panel.check_layout",
    args: {
      text,
      drawn: [...drawn],
      ...(committed === null ? {} : { committed }),
    },
  });

// ---- AI organize ---------------------------------------------------------

/** The per-module half of the prompt: everything that changes between two requests. */
export const aiUserPrompt = (req: AiRequest): string =>
  call<string>({ op: "panel.ai_user_prompt", args: { req } });

/** The builder, the widgets, the screen contract and the design brief: what any answer, API or files, is designed by. */
export const aiDesignBrief = (): string =>
  call<string>({ op: "panel.ai_brief" });

/** Whether a panel file lays anything out: the question that decides whether there is a layout to update. */
export const hasLayout = (uiText: string): boolean =>
  call<boolean>({ op: "panel.has_layout", args: { uiText } });

/** Every bind a proposed panel may carry: the params, and on a rack module its taps. */
export const bindableNames = (req: {
  params: readonly AiRequest["params"][number][];
  moduleText: string;
}): string[] =>
  call<string[]>({
    op: "panel.bindable_names",
    args: { params: req.params, moduleText: req.moduleText },
  });

/** Why a proposal must not land, as sentences the MODEL can act on; empty means it may. */
export const checkAiResult = (
  result: Omit<AiResult, "issues"> & { issues?: readonly string[] },
  bindNames: readonly string[],
  ctx?: Partial<AiContext>,
): Checked =>
  call<Checked>({
    op: "panel.ai_check",
    args: {
      result: { ...result, issues: [...(result.issues ?? [])] },
      bindNames: [...bindNames],
      ctx: ctx ?? {},
    },
  });
