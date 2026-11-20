// protocol.vfs.test.ts — the Worker's protocol over a real MessageChannel: a runtime per `at`, and what survives the clone.
// What a reader gets wrong: a MessageChannel clones as a Worker does, so an array's own `refused` is lost and the field is what is read.

import { MessageChannel } from "node:worker_threads";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { beforeEach, describe, expect, it } from "vitest";
import { provideCommandsYaml, registerVerb } from "../commandSpec.ts";
import { HostError } from "./members/jobs.ts";
import { client, type PortLike, serve } from "./protocol.ts";
import { commandsYamlText } from "./realFs.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { setVfs } from "./shims/current.ts";
import { type FakeServer, fakeServer } from "./testing/fakeServer.ts";
import { Unsupported } from "./unsupported.ts";
import { Vfs } from "./vfs.ts";

const AMP = "@workspace/audio/amp";
const root = "/ws";
let servers: Record<string, FakeServer>;
let page: ReturnType<typeof client>;
let made: Runtime[];

function portOf(p: import("node:worker_threads").MessagePort): PortLike {
  return {
    postMessage: (m) => p.postMessage(m),
    addEventListener: (_t, f) => p.on("message", (data) => f({ data })),
  };
}

beforeEach(() => {
  provideCommandsYaml(commandsYamlText());
  const disk = new Vfs(true);
  setVfs(disk);
  writeDoc(`${root}/_modules/audio/amp/module.k7x`, {
    plugin: { type: "internal", name: "Chorus" },
  });
  for (const name of ["a.k7", "b.k7"]) {
    const dir = `${root}/${name}`;
    writeDoc(`${dir}/project.k7x`, { project: { name, tempo: 120 } });
    writeDoc(`${dir}/graph.k7x`, { modules: { gtr: [AMP], [AMP]: ["OUT"] } });
    writeLane(dir, "gtr", "track", { events: [] });
  }
  servers = {
    "a.k7": fakeServer(disk, root, "a.k7"),
    "b.k7": fakeServer(disk, root, "b.k7"),
  };
  servers["a.k7"]?.serve("engine", async () => ({
    status: 501,
    body: { error: "arm: this front serves no engine", code: "unsupported" },
  }));
  made = [];
  const both = {
    ...servers["a.k7"],
    bundle: (at: string) => servers[at]?.bundle(at) as never,
    changes: (set: { at: string }) =>
      servers[set.at]?.changes(set as never) as never,
  };
  const { port1, port2 } = new MessageChannel();
  serve(portOf(port1), (_init, libraries) => {
    const rt = createRuntime({ remote: both as never, libraries });
    made.push(rt);
    return rt;
  });
  page = client(portOf(port2));
  port1.unref();
  port2.unref();
});

registerVerb(
  { command: "rt needs", sid: "rt.needs", surfaces: ["cli"] },
  () => {
    throw new Unsupported("transport");
  },
);

registerVerb(
  { command: "rt hosterr", sid: "rt.hosterr", surfaces: ["cli"] },
  () => {
    throw new HostError("arm: this verb needs the server", "unsupported");
  },
);

registerVerb(
  { command: "rt caught", sid: "rt.caught", surfaces: ["cli"] },
  async (_args, h) => {
    try {
      await h.arm("gtr");
    } catch {
      // A verb that carries on after the member failed.
    }
    h.log(["done anyway"]);
  },
);

describe("the protocol", () => {
  it("refuses a verb that caught the missing member and went on", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    const r = await page.run("rt caught");
    expect([r.refused, r.unsupported, r.committed]).toEqual([
      true,
      "engine",
      false,
    ]);
    expect([...r.output]).toEqual([
      "rt caught is not available yet: it needs engine",
    ]);
  });

  it("maps a host error coded unsupported to the same refusal", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    const r = await page.run("rt hosterr");
    expect(r.refused).toBe(true);
    expect(r.unsupported).toBe("arm");
  });

  it("answers the catalogue before any project is open", async () => {
    await page.init("http://x");
    const list = await page.catalogue();
    expect(list.some((c) => c.command === "node new")).toBe(true);
  });

  it("keeps a runtime per `at` and runs each over its own project", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    await page.open("b.k7");
    expect(made).toHaveLength(3);
    // No `at` is the project opened last; then `a` again, which must read its own files.
    await page.run("node new Audio/AMP one");
    await page.run("node new Audio/AMP two", undefined, "a.k7");
    await page.run("node new Audio/AMP three");
    const files = (s: FakeServer) =>
      s.sets.flatMap((x) => x.set.ops.map((o) => o.file));
    expect(files(servers["b.k7"] as FakeServer)).toContain(
      "b.k7/nodes/one.k7x",
    );
    expect(files(servers["b.k7"] as FakeServer)).toContain(
      "b.k7/nodes/three.k7x",
    );
    expect(files(servers["a.k7"] as FakeServer)).toContain(
      "a.k7/nodes/two.k7x",
    );
    expect(files(servers["a.k7"] as FakeServer)).not.toContain(
      "a.k7/nodes/one.k7x",
    );
  });

  it("carries `refused` as a field and an unsupported member by name", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    const refused = await page.run("node new Nope/zzz x");
    expect(refused.refused).toBe(true);
    expect(refused.committed).toBe(false);
    const needs = await page.run("rt needs");
    expect(needs.refused).toBe(true);
    expect(needs.unsupported).toBe("transport");
    expect([...needs.output]).toEqual([
      "rt needs is not available yet: it needs transport",
    ]);
    const fine = await page.run("node new Audio/AMP ok");
    expect(fine.refused).toBe(false);
    expect(fine.unsupported).toBeUndefined();
  });

  it("refuses a call before init and a run with no project", async () => {
    await expect(page.open("a.k7")).rejects.toThrow(/initialised/);
    await page.init("http://x");
    await expect(page.run("node new Audio/AMP x")).rejects.toThrow(
      /no project/,
    );
  });

  it("closes one runtime and leaves the others", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    await page.open("b.k7");
    await page.close("b.k7");
    await expect(
      page.run("node new Audio/AMP x", undefined, "b.k7"),
    ).rejects.toThrow();
    expect(
      (await page.run("node new Audio/AMP x", undefined, "a.k7")).committed,
    ).toBe(true);
  });
});
