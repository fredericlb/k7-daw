// testHost.ts — a COMPLETE `CommandHost` for a test, of which a caller states only what matters.
//
// Complete by construction, so forgetting a field is a compile error rather than a surprise. A
// suite building its own and closing it with `as never` switches the typer off: a host missing a
// field the verb calls then breaks at runtime inside a test about something else, and an optional
// field set to `undefined` takes the parameter default, so a "without this capability" test
// exercises a host that has it. Both read as green.

import type {
  AudioDeviceInfo,
  PluginInfo,
} from "@studio/engine-client/engineClient";
import type { ProjectDoc } from "@studio/song/project";
import { wavSeconds } from "@studio/take-render/wavLength";
import type { CommandHost } from "./commands.ts";
import type { VideoFacts } from "./hostTypes.ts";

/** A playable 10 s clip, what the fake probe answers for any file. */
const FACTS: VideoFacts = {
  codec: "h264",
  width: 1920,
  height: 1080,
  pixFmt: "yuv420p",
  fps: 25,
  seconds: 10,
  container: "mov,mp4,m4a,3gp,3g2,mj2",
};

/** Media members that run no tool: the host is a made-up one, the tools are the Rust front's. */
function fakeMedia(): Pick<
  CommandHost,
  "probe" | "extractSound" | "prepare" | "playlistStems"
> {
  return {
    probe: async () => FACTS,
    extractSound: async () => null,
    prepare: async (ask) => ({
      started: true,
      line: `${ask.clip}: prepared (fake)`,
    }),
    playlistStems: async ({ songs }) => ({
      songs: songs.map((song) => ({ song, state: "absent" as const })),
    }),
  };
}

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
    // No engine renders or measures here: a test that wants one passes its own.
    render: undefined,
    peak: undefined,
    seconds: wavSeconds,
    ...fakeMedia(),
    ...over,
  };
  return host;
}
