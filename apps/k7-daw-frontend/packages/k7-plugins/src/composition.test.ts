import { describe, expect, it } from "vitest";
import { BROWSER_PLUGINS, builtinBrowserPlugins } from "./browser";

// The composition list is the whole point of this package — the shell registers it — so the test pins that
// every plugin in it carries the pane drawer the browser half exists for.

describe("the composition list", () => {
  it("holds plugins that each carry a pane drawer", () => {
    for (const p of BROWSER_PLUGINS)
      expect(p.panes?.length ?? 0, p.id).toBeGreaterThan(0);
    expect(builtinBrowserPlugins()).toEqual(BROWSER_PLUGINS);
  });
});
