// palette.ts — the palette's catalogue (the backend's commands plus the shell's own, whose table is ui/commands/) and each argument's vocabulary.
// A reader would get wrong who decides a field's list: Rust says which list it takes (`argSource`), this file fetches or hands in what only the page holds.

import {
  argSource,
  buildCommands,
  type LocalCommand,
  commandNameOf as nameOf,
} from "@studio/k7-wasm/menu";
import { type ArgOption, optionsFromCompletions } from "@studio/k7-wasm/ui";
import type { Completion } from "@studio/project-shell/project";
import { api } from "./api";
import type { PaletteArg, PaletteCommand } from "./components/CommandBar";
import { ROLL_MODES } from "./rollMode";
import { TOOLS } from "./timelineTools";
import type { CommandInfo } from "./types";
import {
  LOCAL_ARGS,
  LOCAL_COMMANDS,
  LOCAL_READABLE,
} from "./ui/commands/index";
import { WAVE_MODES } from "./waveMode";

// The shell's own commands, their sentences and their arguments: one registry, ui/commands/.
export {
  LOCAL_ARGS,
  LOCAL_COMMANDS,
  LOCAL_READABLE,
} from "./ui/commands/index";

/** The palette's full catalogue: the backend's commands plus the shell's own, each description resolved once by `locale`. */
export function buildPaletteCommands(
  backendCmds: CommandInfo[],
  locale: string,
): PaletteCommand[] {
  const locals: LocalCommand[] = LOCAL_COMMANDS.map((name) => ({
    name,
    ...(LOCAL_READABLE[name] ? { readable: LOCAL_READABLE[name] } : {}),
    args: (LOCAL_ARGS[name] ?? []) as LocalCommand["args"],
  }));
  return buildCommands(backendCmds, locals, locale) as PaletteCommand[];
}

/** Which command a canonical LINE runs: the longest name that prefixes it, exactly as the dispatcher picks it. */
export const commandNameOf = (
  line: string,
  commands: readonly PaletteCommand[],
): string | null =>
  nameOf(
    line,
    commands.map((c) => c.canonical),
  );

/** One param's WHOLE vocabulary, given the params before it: fetched whole and ranked in the browser, so the match is fuzzy and costs one round trip. */
export function makeListArg(shell: {
  /** What the editor can OPEN: `ui file`'s vocabulary. */
  files: () => string[];
  /** Every path the tree draws, folders included: what a `file …` verb may name. */
  entries: () => string[];
  /** The surfaces this project declares, by id: `ui external`'s vocabulary, and only this app can enumerate one project's. */
  surfaces: () => string[];
  /** The languages the shell is compiled for. */
  locales: () => string[];
  /** The project's timelines but Main, by id: which one `project export audio` renders. */
  timelines: () => string[];
}) {
  return async (
    command: string,
    priorArgs: string[],
  ): Promise<(string | ArgOption)[]> => {
    const canonical = command.trim();
    const kind = (LOCAL_ARGS[canonical] as PaletteArg[] | undefined)?.[
      priorArgs.length
    ]?.kind;
    const source = argSource(
      canonical,
      priorArgs,
      {
        files: shell.files(),
        entries: shell.entries(),
        surfaces: shell.surfaces(),
        locales: shell.locales(),
        timelines: shell.timelines(),
        tools: [...TOOLS],
        waveModes: [...WAVE_MODES],
        rollModes: [...ROLL_MODES],
      },
      kind === "file",
    );
    if (source.source === "values") return source.values;
    // `completions` answers with whole LINES, so the prefix we sent is what is sliced back off.
    const full = await api
      .complete(source.line)
      .catch(() => [] as Completion[]);
    return optionsFromCompletions(full, source.prefix);
  };
}
