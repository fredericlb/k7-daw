// protocol.vfs.test.ts — the Worker's protocol over a real MessageChannel: a runtime per `at`, and its picture views answered across the clone.
// What a reader gets wrong: a MessageChannel clones as a Worker does, so what crosses is what a real Worker would hand the page.

import { MessageChannel } from "node:worker_threads";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { beforeEach, describe, expect, it } from "vitest";
import { client, type PortLike, serve } from "./protocol.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { setVfs } from "./shims/current.ts";
import { type FakeServer, fakeServer } from "./testing/fakeServer.ts";
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

describe("the protocol", () => {
  it("keeps a runtime per `at`, the one opened last answering a call with none", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    await page.open("b.k7");
    expect(made).toHaveLength(2);
    const a = await page.videoDoc(undefined, "a.k7");
    const b = await page.videoDoc();
    expect("doc" in a && a.doc.videoTracks).toEqual([]);
    expect("doc" in b && b.doc.videoTracks).toEqual([]);
    expect(await page.videoErrors(undefined, "a.k7")).toEqual({
      errors: [],
      diagnostics: [],
    });
  });

  it("refuses a call before init and a view with no project", async () => {
    await expect(page.open("a.k7")).rejects.toThrow(/initialised/);
    await page.init("http://x");
    await expect(page.videoDoc()).rejects.toThrow(/no project/);
  });

  it("closes one runtime and leaves the others", async () => {
    await page.init("http://x");
    await page.open("a.k7");
    await page.open("b.k7");
    await page.close("b.k7");
    await expect(page.videoDoc(undefined, "b.k7")).rejects.toThrow();
    expect("doc" in (await page.videoDoc(undefined, "a.k7"))).toBe(true);
  });
});
