// local.vfs.test.ts — the members the Worker answers itself: applying a project asks for the reload, the chains come from the preferences.

import { stringifyPreferences } from "@studio/song/preferences";
import { describe, expect, it } from "vitest";
import type { CommandHost } from "../../commands.ts";
import type { RunFlags } from "../host.ts";
import { withLocal } from "./local.ts";

const flags = (): RunFlags => ({ reload: false, hostCalled: false });

describe("the Worker's own members", () => {
  it("answers applyProject and sendProject as the reload, and does not call the server", async () => {
    const f = flags();
    const host = withLocal({} as CommandHost, f, null);
    await host.applyProject({} as never);
    expect(f.reload).toBe(true);
    const g = flags();
    await withLocal({} as CommandHost, g, null).sendProject({} as never);
    expect(g).toEqual({ reload: true, hostCalled: false });
  });

  it("reads the separation chains from the bundle's preferences, and says nothing without them", () => {
    const text = stringifyPreferences({
      stemsChain: " mine ",
      instrumentalChain: "karaoke",
    } as never);
    const host = withLocal({} as CommandHost, flags(), text);
    expect([host.stemsChain, host.instrumentalChain]).toEqual([
      "mine",
      "karaoke",
    ]);
    const none = withLocal({} as CommandHost, flags(), null);
    expect([none.stemsChain, none.instrumentalChain]).toEqual([
      undefined,
      undefined,
    ]);
    const broken = withLocal({} as CommandHost, flags(), ":::");
    expect(broken.stemsChain).toBeUndefined();
  });
});
