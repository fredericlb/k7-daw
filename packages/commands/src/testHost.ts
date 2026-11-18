// testHost.ts — a COMPLETE `CommandHost` for a test, of which a caller states only what matters.
//
// Complete by construction, so forgetting a field is a compile error rather than a surprise. A
// suite building its own and closing it with `as never` switches the typer off: a host missing a
// field the verb calls then breaks at runtime inside a test about something else, and an optional
// field set to `undefined` takes the parameter default, so a "without this capability" test
// exercises a host that has it. Both read as green.

import { measureFilePeakDb } from "@studio/audio-passes/peakPass";
import { findEngineBinary } from "@studio/engine-client/engineBinary";
import type {
  AudioDeviceInfo,
  PluginInfo,
} from "@studio/engine-client/engineClient";
import type { ProjectDoc } from "@studio/song/project";
import { localRenderer } from "@studio/take-render/renderer";
import { wavSeconds } from "@studio/take-render/wavLength";
import type { CommandHost } from "./commands.ts";
import { nodeMediaMembers } from "./nodeMedia.ts";

/** What a test reads afterwards: both channels' lines, kept apart as the verb keeps them. */
export interface HostLog {
  log: string[];
  refuse: string[];
}

/** A complete host with INERT defaults — never spies — and an empty project unless one is given. */
export function testHost(
  over: Partial<CommandHost> = {},
): CommandHost & { lines: HostLog } {
  const lines: HostLog = { log: [], refuse: [] };
  const bin = findEngineBinary();
  const host: CommandHost & { lines: HostLog } = {
    lines,
    songPath: "",
    project: () => ({ tracks: [] }) as unknown as ProjectDoc,
    baseModules: [],
    log: (l) => lines.log.push(...l),
    refuse: (l) => lines.refuse.push(...l),
    reload: () => {},
    transport: () => {},
    arm: async () => undefined,
    applyProject: () => {},
    sendProject: () => {},
    listPlugins: () => {},
    rescanPlugins: () => {},
    showNativePluginUI: () => {},
    getPlugins: async () => [] as PluginInfo[],
    getAudioDevices: async () => [] as AudioDeviceInfo[],
    // The engine of this machine, as a server host supplies it; absent with no binary.
    render: bin ? localRenderer(bin) : undefined,
    peak: bin ? (file) => measureFilePeakDb(bin, file) : undefined,
    seconds: wavSeconds,
    // The media members of this machine, reading the host's own background and cold step as it ends up.
    ...nodeMediaMembers({
      background: () => host.background,
      runColdStep: () => host.runColdStep,
    }),
    ...over,
  };
  return host;
}
