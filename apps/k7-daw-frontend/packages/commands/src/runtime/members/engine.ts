// engine.ts — the engine's host members in the Worker: transport, arm, the takes view's arm, a timeline's audition, a plugin's window (3f-1, M9).
// What a reader gets wrong: the lines the server answers (engine offline, an arm that timed out) are pushed to the run's output, never thrown.

import type { CommandHost } from "../../commands.ts";
import type { Output } from "../host.ts";
import { type Hosting, hostCall } from "./jobs.ts";

type EngineMembers = Pick<
  CommandHost,
  "transport" | "arm" | "takeArm" | "timelinePlay" | "showNativePluginUI"
>;

const op = (h: Hosting, body: Record<string, unknown>) =>
  hostCall<{ lines?: string[] } & Record<string, unknown>>(h, "engine", body);

/** The members of one run; each marks the run `hostCalled`. */
export function engineMembers(h: Hosting, out: Output): EngineMembers {
  return {
    transport: async (action) => {
      await op(h, { op: "transport", action });
    },
    arm: async (track, armed) => {
      const a = await op(h, {
        op: "arm",
        track,
        ...(armed === undefined ? {} : { armed }),
      });
      out.push(...(a.lines ?? []));
      return typeof a.armed === "boolean" ? a.armed : undefined;
    },
    takeArm: async (arm) => {
      await op(h, { op: "takeArm", arm });
      return true;
    },
    timelinePlay: async (id) => {
      await op(h, { op: "timelinePlay", id });
      return true;
    },
    showNativePluginUI: async (id) => {
      out.push(...((await op(h, { op: "showUi", id })).lines ?? []));
    },
  };
}
