// Every message key Rust can put in a menu or the home page's catalogue is a message the page holds: a missing one would read as its key.

import type { MenuCtx, Text } from "@studio/k7-wasm/menu";
import { homeCatalogue } from "@studio/k7-wasm/menu";
import { verbCatalogue } from "@studio/k7-wasm/ui";
import { describe, expect, it } from "vitest";
import { contextMenu } from "./contextMenu";
import { hasMessage, words } from "./menuWords";
import { nodeMenu } from "./nodeMenu";
import type { CommandInfo } from "./types";

const keysOf = (t: Text): string[] => (t.key ? [t.key] : []);

describe("the messages Rust names", () => {
  it("are all in the page's catalogue", () => {
    const catalogue = verbCatalogue() as CommandInfo[];
    const contexts: MenuCtx[] = [
      { kind: "track", id: "a", picked: ["a", "b"] },
      { kind: "module", id: "m", model: true, native: true },
      { kind: "module", id: "@engine/x", ambient: true },
      { kind: "clip", id: "c", track: "t", tc: "1:1" },
      {
        kind: "param",
        id: "m",
        param: "p",
        label: "p",
        automated: true,
        cc: { ch: 1, cc: 2 },
      },
      { kind: "curve-card", curve: "c" },
      { kind: "curve-card", curve: "c", module: "m", param: "p" },
      { kind: "assembly", id: "a", uses: "u" },
      { kind: "marker-add", tc: "1:1", loop: "set" },
      { kind: "marker-add", tc: "1:1", loop: "clear" },
      { kind: "picture-lane", lane: "cam" },
      { kind: "picture-lane", lane: "t>cam" },
      { kind: "tap", id: "k", symbol: "s" },
    ];
    const rows = contexts.flatMap((c) => contextMenu(c, catalogue));
    for (const r of rows) expect(r.label, r.verb).not.toBe("");
    const gutter = nodeMenu({
      module: "m",
      alternatives: ["1", "x"],
      alternative: "x",
      frozen: "at",
    });
    expect(gutter.length).toBeGreaterThan(4);
    const keys = homeCatalogue("en").flatMap((r) =>
      r.args.flatMap((a) => keysOf(a.hint)),
    );
    for (const k of keys) expect(hasMessage(k), k).toBe(true);
    for (const r of rows)
      expect(
        r.label.startsWith("ctx.") || r.description.startsWith("ctx."),
        r.verb,
      ).toBe(false);
    for (const g of gutter)
      expect(g.label.startsWith("node."), g.verb).toBe(false);
  });

  it("read as their key when the page lacks one, so a gap is seen", () => {
    expect(words({ key: "no.such.key" })).toBe("no.such.key");
    expect(words({ prefix: "• ", text: "x" })).toBe("• x");
  });
});
