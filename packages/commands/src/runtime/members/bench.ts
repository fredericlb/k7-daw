// bench.ts — `chain save` as a host member: the bench's run written as the account's chain, by 3j's own route (3f, M9).
// What a reader gets wrong: the route is `/bench/save`, outside `/verb-host`; it is given the project's NAME (`at`), never the verb's virtual folder.

import type { CommandHost } from "../../commands.ts";
import { HostError, type Hosting, hostCall } from "./jobs.ts";

type BenchMembers = Pick<CommandHost, "saveBenchChain">;

/** The member of one run; it marks the run `hostCalled`. */
export function benchMembers(h: Hosting): BenchMembers {
  return {
    // The verb's `dir` is the virtual `/w/<at>`: the route resolves `at` to the real bench folder itself.
    saveBenchChain: (_dir, name) => {
      if (h.at === undefined) throw new HostError("no project is open");
      return hostCall<{ ref: string; steps: number; outside: string[] }>(
        h,
        "/bench/save",
        name === undefined ? {} : { name },
      );
    },
  };
}
