// ui.ts — the command bar's, the menus' and the edits' rules from Rust (the `verbs` crate's `ui` module), one sync JSON call each.
// A reader would get wrong that a rule is decided here: these wrap an op and map an answer's indices back onto the caller's own rows.

import { call } from "./abi.ts";
import type {
  ArgOption,
  CaretAim,
  Completion,
  EmptyRow,
  EnterAction,
  FormArg,
  Indices,
  MenuChoiceAnswer,
  Offered,
  PrefsAnswer,
  PrefsView,
  Scores,
  Step,
  StepCommand,
} from "./types/index.ts";

export {
  EMPTY_RECENT_MAX,
  MAX_RECENT,
  MENU_ENTITIES,
} from "./consts.generated.ts";

export type {
  ArgOption,
  CaretAim,
  Completion,
  EmptyRow,
  EnterAction,
  FormArg,
  Offered,
  PrefsView,
  Step,
  StepCommand,
};

/** The page's per-command overrides, keyed by the canonical line (apps/k7-daw-frontend/src/commandPrefs docs). */
export type Prefs = Record<string, object>;

// ---- fuzzy and the rankings ------------------------------------------------

/** Each string's fuzzy score against `q`, in order; -1 is no match. */
export const fuzzyScores = (q: string, strings: readonly string[]): number[] =>
  call<Scores>({ op: "ui.fuzzy_scores", args: { q, strings: [...strings] } })
    .scores;

/** The catalogue ranked against what was typed, on the sentence and the name, recent commands nudged up; at most `limit`. */
export function rankCommands<C extends { canonical: string; readable: string }>(
  commands: readonly C[],
  query: string,
  recent: readonly string[] = [],
  limit = 30,
): C[] {
  const rows = commands.map((c) => ({
    canonical: c.canonical,
    readable: c.readable,
  }));
  return call<Indices>({
    op: "ui.rank_commands",
    args: { commands: rows, query, recent: [...recent], limit },
  }).indices.map((i) => commands[i] as C);
}

/** One value a field offers: what is INSERTED, and what the list shows instead when that is unreadable. */
export const toOptions = (vals: readonly (string | ArgOption)[]): ArgOption[] =>
  vals.map((v) => (typeof v === "string" ? { value: v } : v));

/** A field's options ranked against what was typed, on the value or the label. */
export function rankOptions(
  options: readonly ArgOption[],
  query: string,
  limit = 30,
): ArgOption[] {
  return call<Indices>({
    op: "ui.rank_options",
    args: { options: [...options], query, limit },
  }).indices.map((i) => options[i] as ArgOption);
}

/** `/complete`'s whole lines as one field's options: the prefix sliced back off, deduped, labels kept. */
export const optionsFromCompletions = (
  full: readonly Completion[],
  prefix: string,
): ArgOption[] =>
  call<ArgOption[]>({
    op: "ui.options_from_completions",
    args: { full: [...full], prefix },
  });

/** What the bar lists with nothing typed: recents the catalogue still holds, then the starters it holds. */
export function emptyBarRows<C extends { canonical: string }>(
  commands: readonly C[],
  recent: readonly string[],
  starters?: readonly string[],
): { command: C; recent: boolean }[] {
  const rows = call<EmptyRow[]>({
    op: "ui.empty_bar",
    args: {
      commands: commands.map((c) => c.canonical),
      recent: [...recent],
      ...(starters ? { starters: [...starters] } : {}),
    },
  });
  return rows.map((r) => ({
    command: commands[r.index] as C,
    recent: r.recent,
  }));
}

// ---- the parameter form ----------------------------------------------------

const formArgs = (
  args: readonly { name: string; kind?: string; optional?: boolean }[],
): FormArg[] =>
  args.map((a) => ({
    name: a.name,
    kind: a.kind ?? "",
    optional: a.optional ?? false,
  }));

type ArgLike = { name: string; kind?: string; optional?: boolean };

/** How many params must be filled before the line is runnable. */
export const requiredCount = (args: readonly ArgLike[]): number =>
  call<number>({
    op: "ui.required_count",
    args: { args: formArgs(args), values: [] },
  });

/** A fresh, empty answer sheet: one slot per param. */
export const emptyValues = (args: readonly ArgLike[]): string[] =>
  call<string[]>({
    op: "ui.empty_values",
    args: { args: formArgs(args), values: [] },
  });

/** Whether the line can be dispatched as it stands. */
export const canRun = (
  args: readonly ArgLike[],
  values: readonly string[],
): boolean =>
  call<boolean>({
    op: "ui.can_run",
    args: { args: formArgs(args), values: [...values] },
  });

/** The line's arguments, trailing blanks dropped; a blank in the middle is kept. */
export const lineArgs = (values: readonly string[]): string[] =>
  call<string[]>({ op: "ui.line_args", args: { values: [...values] } });

/** The next field after `from`; the last one stays put. */
export const nextField = (args: readonly ArgLike[], from: number): number =>
  call<number>({
    op: "ui.next_field",
    args: { args: formArgs(args), from },
  });

/** The first field still unanswered, or -1 when the form is complete. */
export const firstMissing = (
  args: readonly ArgLike[],
  values: readonly string[],
): number =>
  call<number>({
    op: "ui.first_missing",
    args: { args: formArgs(args), values: [...values] },
  });

/** Where a gesture's values land, keyed by param name, filling forward only from the active field; null when nothing matches. */
export const applyOffer = (
  args: readonly ArgLike[],
  offered: Readonly<Record<string, string>>,
  values: readonly string[],
  active: number,
): Offered | null =>
  call<Offered | null>({
    op: "ui.apply_offer",
    args: {
      args: formArgs(args),
      offered: { ...offered },
      values: [...values],
      active,
    },
  });

/** The caret's answers to one command, ready for `applyOffer`. */
export const caretAnswers = (
  canonical: string,
  caret: CaretAim | null,
): Record<string, string> =>
  call<Record<string, string>>({
    op: "ui.caret_answers",
    args: { canonical, ...(caret ? { caret } : {}) },
  });

/** Whether the field with the keyboard is asking for `kind`. */
export const wantsKind = (
  args: readonly ArgLike[],
  active: number,
  kind: string,
): boolean =>
  call<boolean>({
    op: "ui.wants_kind",
    args: { args: formArgs(args), active, kind },
  });

/** What Enter does in the parameter form. */
export const enterAction = (
  args: readonly ArgLike[],
  values: readonly string[],
  active: number,
  suggestion: string | undefined,
): EnterAction =>
  call<EnterAction>({
    op: "ui.enter_action",
    args: {
      args: formArgs(args),
      values: [...values],
      active,
      ...(suggestion !== undefined ? { suggestion } : {}),
    },
  });

// ---- the undo journal's sentences ------------------------------------------

/** How ids read as names; absent, an id reads as itself. */
export interface MenuNamers {
  lane?: (id: string) => string;
  clip?: (ref: string) => string;
}

/** A journal step's label as a sentence and its subject; the label as written when it names no command with a sentence. */
export function stepSentence(
  label: string,
  commands: readonly StepCommand[],
  locale: string,
  names: MenuNamers = {},
): string {
  const step = call<Step>({
    op: "ui.step_sentence",
    args: { label, commands: [...commands], locale },
  });
  if (step.sentence === null) return label;
  if (!step.subject) return step.sentence;
  const { kind, value } = step.subject;
  const subject =
    kind === "lane" && names.lane
      ? names.lane(value)
      : kind === "clip" && names.clip
        ? names.clip(value)
        : value;
  return `${step.sentence} · ${subject}`;
}

// ---- what a person made of the catalogue -----------------------------------

type Like = { canonical: string; readable?: string };

const likes = (commands: readonly Like[]): Like[] =>
  commands.map((c) => ({
    canonical: c.canonical,
    ...(c.readable !== undefined ? { readable: c.readable } : {}),
  }));

/** Each command's group, plural flag and label, and every entity present: one call for the whole settings page. */
export const prefsView = (
  commands: readonly Like[],
  prefs: Prefs,
  locale: string,
): PrefsView =>
  call<PrefsView>({
    op: "ui.prefs_view",
    args: { commands: likes(commands), prefs, locale },
  });

/** The catalogue as this person arranged it: placed commands first by `order`, the rest in catalogue order. */
export function orderCommands<T extends Like>(
  commands: readonly T[],
  prefs: Prefs,
): T[] {
  return call<Indices>({
    op: "ui.order_commands",
    args: { commands: likes(commands), prefs },
  }).indices.map((i) => commands[i] as T);
}

/** Move `canonical` one place up or down: it renumbers the whole list. */
export const moveCommand = <P extends Prefs>(
  commands: readonly Like[],
  prefs: P,
  canonical: string,
  by: -1 | 1,
): P =>
  call<PrefsAnswer>({
    op: "ui.move_command",
    args: { commands: likes(commands), prefs, canonical, by },
  }).prefs as P;

/** Set fields of one command's entry; an undefined or empty one is taken back, and an entry left empty is dropped. */
export const setCommandPref = <P extends Prefs>(
  prefs: P,
  canonical: string,
  patch: object,
): P =>
  call<PrefsAnswer>({
    op: "ui.set_command_pref",
    args: {
      prefs,
      canonical,
      patch: Object.fromEntries(
        Object.entries(patch).map(([k, v]) => [k, v === undefined ? null : v]),
      ),
    },
  }).prefs as P;

/** Say where a right-click may offer a command, in the spelling that says the least. */
export const setMenuEntities = <P extends Prefs>(
  prefs: P,
  canonical: string,
  catalogue: readonly string[] | undefined,
  next: readonly string[],
  all: readonly string[],
): P =>
  call<PrefsAnswer>({
    op: "ui.set_menu_entities",
    args: {
      prefs,
      canonical,
      ...(catalogue ? { catalogue: [...catalogue] } : {}),
      next: [...next],
      all: [...all],
    },
  }).prefs as P;

/** Name a command in one language, leaving the others. */
export const setCommandLabel = <P extends Prefs>(
  prefs: P,
  canonical: string,
  locale: string,
  text: string,
): P =>
  call<PrefsAnswer>({
    op: "ui.set_command_label",
    args: { prefs, canonical, locale, text },
  }).prefs as P;

/** Which entities a command is offered on as things stand; null meaning "wherever it fits". */
export const menuChoice = <E extends string = string>(
  pref: object | undefined,
  catalogue: readonly E[] | undefined,
): E[] | null =>
  call<MenuChoiceAnswer>({
    op: "ui.menu_choice",
    args: {
      ...(pref ? { pref } : {}),
      ...(catalogue ? { catalogue: [...catalogue] } : {}),
    },
  }).entities as E[] | null;

/** The verbs catalogue as the palette lists it: every command with each argument class, provider and clue. */
export const verbCatalogue = (): unknown[] =>
  call<unknown[]>({ op: "verb.catalogue" });
