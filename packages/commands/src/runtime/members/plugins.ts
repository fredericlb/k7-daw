// plugins.ts — the engine's plugin catalogue as host members: the list a verb reads, the lines a command prints, a rescan (3f, M9).
// What a reader gets wrong: a server with no engine answers an empty list, so "no plugins cached" is also what an offline engine prints.

import type { PluginInfo } from "@studio/engine-client/engineClient";
import type { CommandHost } from "../../commands.ts";
import type { Output } from "../host.ts";
import { type Hosting, hostCall } from "./jobs.ts";

type PluginMembers = Pick<
  CommandHost,
  "getPlugins" | "listPlugins" | "rescanPlugins"
>;

const mark = (p: PluginInfo) =>
  p.role === "instrument"
    ? "[inst]"
    : p.role === "midiEffect"
      ? "[midi]"
      : "     ";

/** One line a plugin is listed by, as `engine plugins` words it. */
const line = (p: PluginInfo) =>
  `  ${mark(p)} ${p.name}  [${p.format}]  ${p.manufacturer}`;

/** The members of one run; each marks the run `hostCalled`. */
export function pluginMembers(h: Hosting, out: Output): PluginMembers {
  const list = async (filter?: string): Promise<PluginInfo[]> =>
    (
      await hostCall<{ plugins: PluginInfo[] }>(
        h,
        "plugins",
        null,
        filter ? `filter=${encodeURIComponent(filter)}` : "",
      )
    ).plugins;
  return {
    getPlugins: list,
    listPlugins: async (filter) => {
      const plugins = await list(filter);
      if (!plugins.length) out.push("no plugins cached — run: k7 engine scan");
      else out.push(...plugins.map(line));
    },
    rescanPlugins: async (force) => {
      const { plugins } = await hostCall<{ plugins: PluginInfo[] }>(
        h,
        "plugins/rescan",
        force === undefined ? {} : { force },
      );
      out.push(`scanned ${plugins.length} plugin(s)`);
    },
  };
}
