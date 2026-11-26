// commandSpec.ts — the catalogue: commands.yaml maps what you TYPE ("module new") to a SID, its arguments and its menus.
// What a reader gets wrong: nothing here runs a verb; the verbs and their completion are the Rust crate's (docs/verbs.md).

import type { CommandInfo } from "@studio/web-protocol";
import { parse as parseYaml } from "yaml";
import commandsYaml from "../commands.yaml?raw";
import {
  type ArgClass,
  CLASS_HINT,
  PROVIDER_CLASS,
  PROVIDER_HINT,
  PROVIDER_NAMES,
  type ProviderName,
} from "./providerInfo";

// One surface — the web backend dispatches under "cli" too — but it stays a type.
export type Surface = "cli";

// A scalar shape checked before the handler, for ONE-token arguments only — docs/completions.md.
type ArgKind = "timecode" | "number" | "note";

export interface ArgSpec {
  name: string;
  // The value's shape. Absent = free text, validated (if at all) by the handler.
  type?: ArgKind;
  // A named provider, checked against PROVIDERS at load, so a typo is reported rather than completing nothing.
  complete?: ProviderName;
  // …or a literal set written in the yaml, so the palette does not ask for what it already knows.
  values?: string[];
  // A one-line clue about what this argument ACCEPTS — worth writing when the class's own clue doesn't say enough.
  hint?: string;
  // TRAILING args only: the line runs without it. See docs/optionalArgs.md.
  optional?: boolean;
}
/** What a command does, as a sentence, in each language a surface can be asked for. */
interface ReadableDescription {
  en: string;
  fr: string;
}

export interface CommandSpec {
  command: string; // the typed prefix (may contain spaces/colons)
  sid: string;
  help?: string;
  // The palette's line, and what it fuzzy-matches on. See commands.yaml's header.
  readable?: ReadableDescription;
  surfaces: Surface[];
  /** Which right-click menus may offer it. Absent ⇒ every one whose click answers its leading args. */
  menu?: MenuEntity[];
  /** The menu asks before running it: what it destroys is more than one Ctrl+Z is trusted to bring back. */
  confirm?: boolean;
  // No fixed leading args: a verb that is one setting of another is the ARG's job — docs/completions.md.
  args?: ArgSpec[]; // trailing user args (for completion)
}

/** The `menu:` words of one entry, unknown ones reported and dropped — a typo would silence a menu. */
function menuList(command: string, words: unknown[]): MenuEntity[] {
  const kept: MenuEntity[] = [];
  for (const w of words.map(String))
    if (MENU_NAMES.has(w)) kept.push(w as MenuEntity);
    else
      console.error(
        `commands.yaml: "${command}" menu: names an unknown entity "${w}" — ` +
          `known: ${MENU_ENTITIES.join(", ")}`,
      );
  return kept;
}

let cached: CommandSpec[] | null = null;

export function loadCommandSpecs(): CommandSpec[] {
  if (cached) return cached;
  try {
    const raw = parseYaml(commandsYaml);
    cached = (Array.isArray(raw) ? raw : [])
      .filter((c: any) => c?.command && c?.sid)
      .map((c: any) => ({
        command: String(c.command),
        sid: String(c.sid),
        help: c.help ? String(c.help) : undefined,
        // Both languages or neither, so a French list never shows one English row in it.
        readable:
          c.readable?.en && c.readable?.fr
            ? { en: String(c.readable.en), fr: String(c.readable.fr) }
            : undefined,
        // An entry that names no surfaces runs everywhere.
        surfaces: Array.isArray(c.surfaces)
          ? (c.surfaces as Surface[])
          : ["cli"],
        // …and one that names no menu is offered on every menu its arguments fit.
        menu: Array.isArray(c.menu)
          ? menuList(String(c.command), c.menu as unknown[])
          : undefined,
        confirm: c.confirm === true ? true : undefined,
        args: Array.isArray(c.args)
          ? c.args.map((a: any) => {
              // An unknown provider is a typo: reported and dropped, never thrown — a throw here would empty the WHOLE catalogue.
              const want = a?.complete ? String(a.complete) : undefined;
              if (want && !PROVIDER_NAMES.has(want))
                console.error(
                  `commands.yaml: "${String(c.command)}" arg "${String(a?.name ?? "")}" ` +
                    `names an unknown completion provider "${want}" — known: ${[...PROVIDER_NAMES].join(", ")}`,
                );
              return {
                name: String(a?.name ?? ""),
                type: argKind(String(c.command), a),
                complete:
                  want && PROVIDER_NAMES.has(want)
                    ? (want as ProviderName)
                    : undefined,
                values: Array.isArray(a?.values)
                  ? a.values.map(String)
                  : undefined,
                hint: a?.hint ? String(a.hint) : undefined,
                optional: a?.optional === true ? true : undefined,
              };
            })
          : undefined,
      }));
  } catch {
    cached = [];
  }
  return cached;
}

// Where a right-click may offer a command, mirrored on the wire — apps/k7-daw-frontend/docs/contextMenu.md.
export const MENU_ENTITIES = [
  "track",
  "group",
  "node",
  "event",
  "param",
  "curve",
  "ruler",
  "marker",
  "timeline",
  "assembly",
  "warp",
  "shot",
  "picture",
  "note",
  "tap",
  "rack",
] as const;
type MenuEntity = (typeof MENU_ENTITIES)[number];
const MENU_NAMES: ReadonlySet<string> = new Set(MENU_ENTITIES);

export function argClass(a: ArgSpec): ArgClass {
  if (a.complete) return PROVIDER_CLASS[a.complete];
  if (a.values) return "choice";
  return a.type ?? "text";
}

// The clue beside the field: the argument's own, else its provider's, else its set, else its class. Empty ⇒ none.
export function argHint(a: ArgSpec): string {
  if (a.hint) return a.hint;
  if (a.complete) return PROVIDER_HINT[a.complete];
  if (a.values) return a.values.join(" · ");
  return CLASS_HINT[argClass(a)];
}

export const ARG_KIND_NAMES: ReadonlySet<string> = new Set([
  "timecode",
  "number",
  "note",
]);

// Read an argument's declared shape; an unknown one is reported and KEPT, so the test suite fails on the typo.
function argKind(
  command: string,
  a: { name?: unknown; type?: unknown },
): ArgKind | undefined {
  if (a?.type === undefined) return undefined;
  const kind = String(a.type);
  if (!ARG_KIND_NAMES.has(kind))
    console.error(
      `commands.yaml: "${command}" arg "${String(a?.name ?? "")}" names an ` +
        `unknown type "${kind}" — known: ${[...ARG_KIND_NAMES].join(", ")}`,
    );
  return kind as ArgKind;
}

/** The palette's catalogue: every command with each argument's display class, provider and clue. */
export function catalogue(): CommandInfo[] {
  return loadCommandSpecs()
    .filter((c) => c.surfaces.includes("cli"))
    .map((c) => ({
      command: c.command,
      help: c.help,
      readable: c.readable,
      menu: c.menu,
      confirm: c.confirm,
      args:
        c.args?.map((a) => ({
          name: a.name,
          kind: argClass(a),
          provider: a.complete,
          hint: argHint(a),
          optional: a.optional,
        })) ?? [],
    }));
}
