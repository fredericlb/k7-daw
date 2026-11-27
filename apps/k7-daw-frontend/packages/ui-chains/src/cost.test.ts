// A temporary measure of what each call costs: the new ops against the TypeScript twins, on a real graph and a large one.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { WebGraph } from "@studio/graph-model";
import { insertsFor, jackRefOp, nodeSizeOp } from "@studio/k7-wasm/graph";
import { it } from "vitest";
import { buildGraph as oldBuild } from "./__old__/buildGraph";
import { moduleRow as oldRow } from "./__old__/moduleRow";
import { stackLayout as oldStack } from "./__old__/stackLayout";
import { buildGraph } from "./buildGraph";
import { moduleRow } from "./moduleRow";
import { stackLayout } from "./stackLayout";

const golden = join(__dirname, "../../../../k7-daw-backend/fixtures/golden");
const load = (f: string): WebGraph =>
  JSON.parse(readFileSync(join(golden, f), "utf8")).graph;

/** The same graph `copies` times over, each copy's names suffixed, so the size grows and nothing is shared. */
function grow(g: WebGraph, copies: number): WebGraph {
  const out: WebGraph = {
    ...g,
    chains: {},
    tracks: [],
    curves: {},
    trackColors: {},
    trackSignals: {},
    modules: {},
  };
  for (let i = 0; i < copies; i++) {
    const s = (n: string) => `${n}~${i}`;
    for (const [name, c] of Object.entries(g.chains))
      out.chains[s(name)] = {
        ...c,
        from:
          c.from && !["MIX", "RENDER"].includes(c.from) ? s(c.from) : c.from,
        use: c.use.map((x) => ({
          ...x,
          module: s(x.module),
          automations: undefined,
        })),
        out: c.out.map((o) => ({
          ...o,
          to: ["OUT", "MIX", "RENDER", "VIDEO_OUT"].includes(o.to)
            ? o.to
            : s(o.to),
        })),
      };
    for (const t of g.tracks) out.tracks.push(s(t));
  }
  return out;
}

const median = (xs: number[]) =>
  xs.sort((a, b) => a - b)[Math.floor(xs.length / 2)];
function time(label: string, f: () => unknown, n = 15) {
  for (let i = 0; i < 3; i++) f();
  const ts: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = performance.now();
    f();
    ts.push(performance.now() - t);
  }
  return `${label}: ${median(ts).toFixed(2)} ms`;
}

it("measures", () => {
  const lines: string[] = [];
  const cases: [string, WebGraph][] = [
    ["real: shapes/mix (26 steps)", load("shapes/mix.json")],
    [
      "real: k7-core overdrive (18 steps)",
      load("k7-core/audio.overdrive.json"),
    ],
    ["large: mix x 50 (1300 steps)", grow(load("shapes/mix.json"), 50)],
  ];
  for (const [name, g] of cases) {
    const steps = Object.values(g.chains).reduce((n, c) => n + c.use.length, 0);
    lines.push(
      `${name}: ${steps} steps, graph JSON ${(JSON.stringify(g).length / 1024).toFixed(0)} KB`,
    );
    const b = buildGraph(g, { drawMainOut: true });
    if (name.startsWith("large"))
      writeFileSync(
        "/private/tmp/claude-502/-Users-mahrave-Src-k7--claude-worktrees-serverless-continuation-06f571/c171d229-da88-4b4c-825c-cf89c1295294/scratchpad/large.json",
        JSON.stringify(g),
      );
    lines.push(
      `  built: ${b.nodes.length} nodes, ${b.edges.length} edges, JSON back ${(JSON.stringify(b).length / 1024).toFixed(0)} KB`,
    );
    lines.push(
      "  " +
        time("buildGraph  old TS", () => oldBuild(g, { drawMainOut: true })),
    );
    lines.push(
      "  " +
        time("buildGraph  Rust ", () => buildGraph(g, { drawMainOut: true })),
    );
    const mods = [
      ...new Set(
        Object.values(g.chains).flatMap((c) => c.use.map((s) => s.module)),
      ),
    ].slice(0, 8);
    lines.push(
      "  " +
        time("moduleRow   old TS", () =>
          oldRow(g, mods, { links: true, asDrawn: true }),
        ),
    );
    lines.push(
      "  " +
        time("moduleRow   Rust ", () =>
          moduleRow(g, mods, { links: true, asDrawn: true }),
        ),
    );
    lines.push(
      "  " +
        time("stackLayout old TS", () =>
          oldStack(b.nodes, b.edges, g, g.tracks),
        ),
    );
    lines.push(
      "  " +
        time("stackLayout Rust ", () =>
          stackLayout(b.nodes, b.edges, g, g.tracks),
        ),
    );
    const pairs = b.edges
      .filter((e) => !e.automation)
      .map((e) => [e.source, e.target] as const);
    lines.push(
      "  " +
        time(`insertsFor ${pairs.length} cables, one call`, () =>
          insertsFor(b.nodes, pairs),
        ),
    );
    const one = b.nodes.find((n) => n.kind === "module");
    if (one) {
      lines.push(
        "  " + time("jackRef one click", () => jackRefOp(one, "a"), 200),
      );
      const ui = one.ui;
      lines.push(
        "  " + time("nodeSize one panel (uncached)", () => nodeSizeOp(ui), 200),
      );
    }
  }
  console.log("\n" + lines.join("\n"));
});
