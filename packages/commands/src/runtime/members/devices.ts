// devices.ts — the audio interfaces the engine sees, and a pair of inputs made one stereo device, as host members (3f, M9).
// What a reader gets wrong: `setInputStereo` answers whether an engine heard the frame, so with none it is false, never true.

import type { AudioDeviceInfo } from "@studio/engine-client/engineClient";
import type { CommandHost } from "../../commands.ts";
import { type Hosting, hostCall } from "./jobs.ts";

type DeviceMembers = Pick<CommandHost, "getAudioDevices" | "setInputStereo">;

/** The members of one run; each marks the run `hostCalled`. */
export function deviceMembers(h: Hosting): DeviceMembers {
  return {
    getAudioDevices: async () =>
      (await hostCall<{ devices: AudioDeviceInfo[] }>(h, "devices", null))
        .devices,
    setInputStereo: async (channel, stereo) =>
      (
        await hostCall<{ heard: boolean }>(h, "devices/stereo", {
          channel,
          stereo,
        })
      ).heard === true,
  };
}
