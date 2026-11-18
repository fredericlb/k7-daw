// library.ts — the account's remote members: what the machine separates on (3f, M9).

import type { CommandHost } from "../../commands.ts";
import { HostError, type Hosting, hostCall } from "./jobs.ts";

type RemoteMembers = Pick<CommandHost, "onnxProviders">;

/** The codes that say nobody holds the setting: this front serves no engine, or the machine could not be reached. */
const NO_ONE = ["off", "no-machine"];

/** The members of one run; each marks the run `hostCalled`. */
export function libraryMembers(h: Hosting): RemoteMembers {
  return {
    // No engine and no machine are the verb's own `null`: it words them, so the sentence is its refusal.
    onnxProviders: async (set) => {
      try {
        return (
          await hostCall<{ providers: string[] | null }>(
            h,
            "onnx",
            set ? { set: [...set] } : {},
          )
        ).providers;
      } catch (e) {
        if (e instanceof HostError && NO_ONE.includes(e.code ?? ""))
          return null;
        throw e;
      }
    },
  };
}
