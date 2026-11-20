// runtime.test.ts — the runtime end to end over a folder: bundle, WASM compile, a verb, its change set, the 409 rerun.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { modelsRoot } from "@studio/song/modelCache";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { wsData } from "@studio/song/yaml/workspaceDoc";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { provideCommandsYaml, registerVerb } from "../commandSpec.ts";
import { makeBrowserHost } from "./host.ts";
import { commandsYamlText } from "./realFs.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { setVfs, withVfs } from "./shims/current.ts";
import { type FakeServer, fakeServer } from "./testing/fakeServer.ts";
import { Vfs } from "./vfs.ts";

const AMP = "@workspace/audio/amp";
const root = "/ws";
const dir = join(root, "song.k7");
let disk: Vfs;
let server: FakeServer;
let rt: Runtime;

beforeEach(async () => {
  provideCommandsYaml(commandsYamlText());
  disk = new Vfs(true);
  setVfs(disk);
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  writeDoc(join(dir, "graph.k7x"), { modules: { gtr: [AMP], [AMP]: ["OUT"] } });
  writeLane(dir, "gtr", "track", { events: [] });
  writeDoc(join(root, "_modules", "audio", "amp", "module.k7x"), {
    plugin: { type: "internal", name: "Chorus" },
  });
  server = fakeServer(disk, root, "song.k7");
  rt = createRuntime({ remote: server });
  await rt.open("song.k7");
});
afterEach(() => {
  rt.close();
});

const writes = (i = 0) =>
  server.sets[i]?.set.ops
    .filter((o) => o.kind !== "read")
    .map((o) => o.file)
    .sort();

// Two test verbs (virtual /w paths): one reads another project's file (outside the bundle), one asks the server for work.
let verbsRegistered = false;
function registerTestVerbs(): void {
  if (verbsRegistered) return;
  verbsRegistered = true;
  registerVerb(
    { command: "rt peek", sid: "rt.peek", surfaces: ["cli"] },
    (_args, h) => {
      h.log([
        `peek: ${readFileSync("/w/other.k7/project.k7x", "utf8").length}`,
      ]);
      writeFileSync(join(h.songPath, "peeked.txt"), "x");
    },
  );
  registerVerb(
    { command: "rt swallow", sid: "rt.swallow", surfaces: ["cli"] },
    (_args, h) => {
      try {
        readFileSync("/w/other.k7/project.k7x", "utf8");
      } catch {
        // A verb that catches every read error must still be rerun on a read the bundle could not answer.
      }
      h.log([existsSync("/w/other.k7/project.k7x") ? "yes" : "no"]);
    },
  );
  registerVerb(
    { command: "rt apply", sid: "rt.apply", surfaces: ["cli"] },
    async (_args, h) => {
      writeFileSync(join(h.songPath, "applied.txt"), "x");
      await h.applyProject(h.project());
    },
  );
  registerVerb(
    { command: "rt host", sid: "rt.host", surfaces: ["cli"] },
    async (_args, h) => {
      writeFileSync(join(h.songPath, "host.txt"), "x");
      await h.arm("gtr");
    },
  );
}

describe("a library module", () => {
  afterEach(() => vi.unstubAllEnvs());

  it("shows the versions the server says are installed", async () => {
    vi.stubEnv("STUDIO_LIBRARIES_HOME", "/libs");
    const disk2 = new Vfs(true);
    setVfs(disk2);
    writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
    writeDoc(join("/libs", "k7-core", "modules", "audio", "x", "module.k7x"), {
      plugin: { type: "internal", name: "Chorus" },
    });
    const srv = fakeServer(disk2, root, "song.k7");
    const withVersions = {
      ...srv,
      bundle: async (at: string) => ({
        ...(await srv.bundle(at)),
        installed: {
          "k7-core": [
            { version: 1, requires: 1 },
            { version: 4, requires: 1 },
          ],
        },
      }),
    };
    const rt2 = createRuntime({ remote: withVersions as never });
    await rt2.open("song.k7");
    const r = await rt2.run("library versions");
    expect(r.refused).toBe(false);
    expect(r.output.join("\n")).toMatch(
      /k7-core[^\n]*1, 4|k7-core[^\n]*1[^\n]*4/,
    );
    rt2.close();
  });

  it("is a module the project sees, so a node is made on its ref", async () => {
    vi.stubEnv("STUDIO_LIBRARIES_HOME", "/libs");
    const disk2 = new Vfs(true);
    setVfs(disk2);
    writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
    writeDoc(join(dir, "graph.k7x"), { modules: { gtr: ["OUT"] } });
    writeLane(dir, "gtr", "track", { events: [] });
    writeDoc(
      join("/libs", "k7-core", "modules", "audio", "tremolo", "module.k7x"),
      {
        plugin: { type: "internal", name: "Chorus" },
      },
    );
    const srv = fakeServer(disk2, root, "song.k7");
    const rt2 = createRuntime({ remote: srv });
    await rt2.open("song.k7");
    const r = await rt2.run(
      "node new audio/tremolo MIX_trem @k7-core/audio/tremolo",
    );
    expect(r.output.join("\n"), JSON.stringify(r.output)).not.toMatch(
      /not a module this project can see/,
    );
    expect(r.committed).toBe(true);
    rt2.close();
  });
});

describe("a path below a folder the listings say is not there", () => {
  it("is absent, not unknown", () => {
    const v = new Vfs();
    v.loadList("/w", [{ name: "p.k7", kind: "dir" }]);
    v.loadList("/w/p.k7", [{ name: "nodes", kind: "dir" }]);
    v.loadList("/w/p.k7/nodes", [{ name: "a.k7x", kind: "file" }]);
    expect(v.stat("/w/p.k7/nodes/base/deep/x")).toBeUndefined();
    expect(v.stat("/w/p.k7/other/x")).toBeUndefined();
    // A listed folder the bundle did not hold is still unknown.
    v.loadList("/w/p.k7", [
      { name: "nodes", kind: "dir" },
      { name: "held", kind: "dir" },
    ]);
    expect(() => v.stat("/w/p.k7/held/x")).toThrow();
  });
});

describe("the runtime", () => {
  it("runs a verb over the vfs and lands its writes as one change set", async () => {
    const r = await rt.run("node new Audio/AMP hall");
    expect(r.output).toEqual([
      "created nodes/hall.k7x — in the graph, wired to nothing yet",
    ]);
    expect(r.runs).toBe(1);
    expect(r.committed).toBe(true);
    expect(server.sets).toHaveLength(1);
    expect(writes()).toEqual(["song.k7/graph.k7x", "song.k7/nodes/hall.k7x"]);
    // The module the verb read outside the project is checked too.
    const read = server.sets[0]?.set.ops.find((o) => o.kind === "read");
    expect(read?.file).toBe("_modules/audio/amp/module.k7x");
    expect(typeof read?.was).toBe("string");
    expect(
      withVfs(disk, () => wsData(join(dir, "nodes", "hall.k7x")).extends),
    ).toBe(AMP);
    expect(r.after).toBe(server.token());
  });

  it("sends reload when a verb applies the project, and runs without the server", async () => {
    registerTestVerbs();
    const r = await rt.run("rt apply");
    expect(r.committed).toBe(true);
    expect(server.sets[0]?.set.reload).toBe(true);
  });

  it("writes nothing when a verb refuses", async () => {
    const before = server.token();
    const r = await rt.run("node new Nope/zzz x");
    expect(r.output.refused).toBe(true);
    expect(r.committed).toBe(false);
    expect(server.sets).toHaveLength(0);
    expect(server.token()).toBe(before);
  });

  it("reruns once by itself when the project moved under it, and lands both edits", async () => {
    server.beforeNextChange(() => {
      writeDoc(join(dir, "graph.k7x"), {
        modules: { gtr: [AMP], [AMP]: ["OUT"], peer: [AMP] },
      });
    });
    const r = await rt.run("node new Audio/AMP hall");
    expect(r.committed).toBe(true);
    expect(r.runs).toBe(2);
    expect(server.sets.map((x) => x.status)).toEqual([409, 200]);
    const modules = withVfs(
      disk,
      () =>
        wsData(join(dir, "graph.k7x")) as { modules: Record<string, string[]> },
    ).modules;
    expect(Object.keys(modules)).toEqual(
      expect.arrayContaining(["peer", "hall"]),
    );
  });

  it("says the project moved when the verb had called the server, and writes nothing", async () => {
    registerTestVerbs();
    server.beforeNextChange(() =>
      writeDoc(join(dir, "graph.k7x"), {
        modules: { gtr: [AMP], [AMP]: ["OUT"], peer: [AMP] },
      }),
    );
    server.serve("engine", async () => ({
      status: 200,
      body: { armed: true },
    }));
    const r = await rt.run("rt host");
    expect(r.committed).toBe(false);
    expect(r.output.join("\n")).toContain("the project moved");
    // The one set it posted was refused: nothing landed.
    expect(server.sets.map((x) => x.status)).toEqual([409]);
  });

  it("fetches a file the bundle lacks and runs the verb again", async () => {
    registerTestVerbs();
    withVfs(disk, () =>
      writeDoc("/ws/other.k7/project.k7x", {
        project: { name: "o", tempo: 90 },
      }),
    );
    const r = await rt.run("rt peek");
    expect(r.runs).toBe(2);
    expect(server.calls.file).toBeGreaterThan(0);
    expect(r.output[0]).toMatch(/^peek: \d+$/);
    expect(r.committed).toBe(true);
  });

  it("reruns a verb that swallowed the read the bundle could not answer", async () => {
    registerTestVerbs();
    withVfs(disk, () =>
      writeDoc("/ws/other.k7/project.k7x", {
        project: { name: "o", tempo: 90 },
      }),
    );
    const r = await rt.run("rt swallow");
    expect(r.output).toEqual(["yes"]);
    expect(r.runs).toBe(2);
  });

  it("never reruns on a 409 that says another project is open", async () => {
    let calls = 0;
    const other = createRuntime({
      remote: {
        ...server,
        bundle: (at) => server.bundle(at),
        library: (a, n, h) => server.library(a, n, h),
        file: (rel) => server.file(rel),
        changes: async () => {
          calls++;
          return {
            status: 409,
            body: {
              error: "not the open project",
              open: "other.k7",
              project: false,
              stale: [],
            },
          };
        },
      },
    });
    await other.open("song.k7");
    await expect(other.run("node new Audio/AMP hall")).rejects.toThrow(
      "not the open project",
    );
    expect(calls).toBe(1);
    other.close();
  });

  it("completes from the vfs, and fetches what the provider needs", async () => {
    const names = await rt.complete("track rename g");
    expect(names.some((x) => String(x).includes("gtr"))).toBe(true);
  });

  it("answers the catalogue from commands.yaml", () => {
    expect(rt.catalogue().some((c) => c.command === "node new")).toBe(true);
  });
});

let modelVerb = false;

describe("the models folder", () => {
  const home = "/ws-home";
  afterEach(() => vi.unstubAllEnvs());

  /** A runtime over a project whose account holds (or not) a model in its models folder, and a verb asking where models are. */
  async function modelRuntime(held: boolean): Promise<Runtime> {
    vi.stubEnv("STUDIO_HOME", home);
    vi.stubEnv("STUDIO_MODELS", "");
    setVfs(disk);
    if (held)
      disk.load(join(home, "models", "htdemucs.onnx"), {
        len: 4,
        binary: true,
      });
    if (!modelVerb) {
      modelVerb = true;
      registerVerb(
        { command: "rt models", sid: "rt.models", surfaces: ["cli"] },
        (_args, h) => {
          h.log([
            `${modelsRoot()} ${existsSync(join(modelsRoot(), "htdemucs.onnx"))}`,
          ]);
        },
      );
    }
    const model = createRuntime({ remote: fakeServer(disk, root, "song.k7") });
    await model.open("song.k7");
    return model;
  }

  it("finds a model the bundle lists, where the Worker has no home to look in", async () => {
    const model = await modelRuntime(true);
    const r = await model.run("rt models");
    expect(r.output).toEqual(["/s/models true"]);
    model.close();
  });

  it("says nothing is there when the folder does not hold it, and leaves the env as it found it", async () => {
    const model = await modelRuntime(false);
    const r = await model.run("rt models");
    expect(r.output).toEqual(["/s/models false"]);
    expect(process.env.STUDIO_MODELS).toBe("");
    model.close();
  });
});

describe("the browser host's seconds", () => {
  it("answers a wav's length as the bundle stated it, and null for a file with none", () => {
    const vfs = new Vfs(true);
    vfs.load("/w/song.k7/a.wav", { len: 8, binary: true, seconds: 3.5 });
    vfs.load("/w/song.k7/b.wav", { len: 8, binary: true });
    const host = makeBrowserHost({ vfs } as never, [], {
      reload: false,
      hostCalled: false,
    });
    expect(host.seconds?.("/w/song.k7/a.wav")).toBe(3.5);
    expect(host.seconds?.("/w/song.k7/b.wav")).toBeNull();
  });
});
