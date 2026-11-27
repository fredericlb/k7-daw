// commandPrefs.ts — the shape of what a person made of the command catalogue: names, icons, order, menus.
// A reader would get wrong that the rules are here: the ordering, the labels and the menu choice are Rust's (`@studio/k7-wasm/ui`).

import type { MenuEntity } from "./types";

/** What a person calls a command, per language. */
export type CommandLabel = Record<string, string>;

/** Where a right-click may offer a command: a list of entities, `false` for nowhere at all. */
type MenuChoice = boolean | MenuEntity[];

/** One command's overrides. Every field optional: an untouched command has no entry at all. */
export interface CommandPref {
  /** What to call it, in each language: a rename is a sentence and a sentence is translated. */
  label?: CommandLabel;
  /** A lucide icon name, e.g. "Scissors". Absent means no icon; an unknown one draws nothing. */
  icon?: string;
  /** Where it sits in a menu. Absent means after everything ordered, in catalogue order. */
  order?: number;
  /** The key that runs it, or KEY_NONE to take a default away; absent means the app's own binding. */
  key?: string;
  /** Which right-clicks may offer it, replacing the catalogue's own list; `false` is none. */
  menu?: MenuChoice;
}

/** Keyed by the canonical line and never by what is displayed: the one id both halves of the catalogue have. */
export type CommandPrefs = Record<string, CommandPref>;

/** A person's name for a command in one locale, as authored: never trimmed here, which is what broke the space bar on the settings page. */
export const labelIn = (
  label: CommandLabel | undefined,
  locale: string,
): string => label?.[locale] ?? "";
