// local.ts — the members the Worker answers itself: applying a project is the reload, the separation chains come from the bundle's preferences.
// What a reader gets wrong: `localMembers` stays empty; `withLocal` is laid over the host last, since the host's stubs come after the spread.

import { type Preferences, parsePreferences } from "@studio/song/preferences";
import type { CommandHost } from "../../commands.ts";
import type { RunFlags } from "../host.ts";

export const localMembers: Partial<CommandHost> = {};

/** The members the Worker answers itself, laid over the host: the project is already in the files, so applying it is the reload. */
export function withLocal(
  host: CommandHost,
  flags: RunFlags,
  preferences: string | null,
): CommandHost {
  const prefs = (): Preferences | undefined => {
    if (preferences === null) return undefined;
    try {
      return parsePreferences(preferences);
    } catch {
      return undefined;
    }
  };
  const reload = (): void => {
    flags.reload = true;
  };
  return Object.defineProperties(host, {
    applyProject: { value: reload, enumerable: true, writable: true },
    sendProject: { value: reload, enumerable: true, writable: true },
    stemsChain: {
      get: () => prefs()?.stemsChain,
      enumerable: true,
      configurable: true,
    },
    instrumentalChain: {
      get: () => prefs()?.instrumentalChain,
      enumerable: true,
      configurable: true,
    },
  });
}
