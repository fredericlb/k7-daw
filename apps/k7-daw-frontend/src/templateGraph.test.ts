// The TypeScript half of templateGraph is the call; the drawing and the refusals are Rust's (k7-wasm tests_template.rs).

import { describe, expect, it } from "vitest";
import { templateGraph } from "./templateGraph";

describe("templateGraph", () => {
  it("reaches the module and answers the graph or the reason", () => {
    const r = templateGraph(
      "@k7-core/x",
      'steps([\n  id("a").extends("@k7-core/audio/eq"),\n  to([chain("@out")]),\n])\n',
    );
    if (!("graph" in r)) throw new Error(r.error);
    expect(r.graph.chains["@k7-core/x"].from).toBe("@in");
    expect(templateGraph("x", "steps([3])\n")).toHaveProperty("error");
  });
});
