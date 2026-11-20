// runtime.host.vfs.test.ts — a job's files through the runtime: staged by a job, adopted by a verb, landed by one change set.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { provideCommandsYaml, registerVerb } from "../commandSpec.ts";
import { adoptFiles, hostJob } from "./members/jobs.ts";
import { commandsYamlText } from "./realFs.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { current, setVfs, withVfs } from "./shims/current.ts";
import { type FakeServer, fakeServer } from "./testing/fakeServer.ts";
import { Vfs } from "./vfs.ts";

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
  writeDoc(join(dir, "graph.k7x"), { modules: {} });
  writeLane(dir, "gtr", "track", { events: [] });
  server = fakeServer(disk, root, "song.k7");
  rt = createRuntime({ remote: server });
  await rt.open("song.k7");
  registerTestVerbs();
});
afterEach(() => rt.close());

let registered = false;
const trace: string[] = [];
function registerTestVerbs(): void {
  if (registered) return;
  registered = true;
  // Asks the runtime's own `peak` member: the flag a 409 reads is the runtime's, not the verb's.
  registerVerb(
    { command: "rth peak", sid: "rth.peak", surfaces: ["cli"] },
    async (args, h) => {
      trace.push(`start ${args}`);
      const db = await h.peak?.("/ws/song.k7/a.wav");
      trace.push(`end ${args}`);
      h.log([`peak ${db}`]);
      // A write, so the run commits and a moved project is a 409.
      current().writeText(`${h.songPath}/note-${args}.txt`, "x");
    },
  );
  // Renders through the server into /tmp, then copies the mix to the project: the freeze's own shape.
  registerVerb(
    { command: "rth freeze", sid: "rth.freeze", surfaces: ["cli"] },
    async (_args, h) => {
      const flags = { reload: false, hostCalled: false };
      const { job, value } = await hostJob<{
        files: { out: string; len: number; seconds: number }[];
      }>({ remote: server, flags, pollMs: 1 }, "render", { at: "song.k7" });
      adoptFiles(current(), job, value.files, (out) => `/tmp/r1/${out}`);
      current().copy("/tmp/r1/mix.wav", `${h.songPath}/mix.wav`);
      h.log(["frozen"]);
    },
  );
  registerVerb(
    { command: "rth boom", sid: "rth.boom", surfaces: ["cli"] },
    async (_args, h) => {
      const flags = { reload: false, hostCalled: false };
      const { job, value } = await hostJob<{
        files: { out: string; len: number }[];
      }>({ remote: server, flags, pollMs: 1 }, "render", { at: "song.k7" });
      adoptFiles(current(), job, value.files, (out) => `${h.songPath}/${out}`);
      throw new Error("failed after the job");
    },
  );
}

describe("a job's files through the runtime", () => {
  it("lands the adopted mix by one change set that names the staged output", async () => {
    const job = server.startJob([
      {
        state: "done",
        value: { files: [{ out: "mix.wav", len: 4, seconds: 0.5 }] },
      },
    ]);
    server.stage(job, "mix.wav", "RIFF");
    server.serve("render", () => ({ status: 202, body: { job } }));
    current().mkdir("/tmp/r1", true);
    const r = await rt.run("rth freeze");
    expect(r.committed).toBe(true);
    const ops = server.sets[0]?.set.ops.filter((o) => o.kind !== "read");
    expect(ops).toEqual([
      { kind: "adopt", job, out: "mix.wav", file: "song.k7/mix.wav" },
    ]);
    expect(
      withVfs(disk, () => readFileSync(join(dir, "mix.wav"), "utf8")),
    ).toBe("RIFF");
  });

  it("lands nothing when the verb fails after the job", async () => {
    const job = server.startJob([
      { state: "done", value: { files: [{ out: "mix.wav", len: 4 }] } },
    ]);
    server.stage(job, "mix.wav", "RIFF");
    server.serve("render", () => ({ status: 202, body: { job } }));
    current().mkdir("/tmp/r1", true);
    const r = await rt.run("rth boom");
    expect(r.committed).toBe(false);
    expect(r.output.join("\n")).toContain("after the job");
    expect(server.sets).toHaveLength(0);
    expect(withVfs(disk, () => existsSync(join(dir, "mix.wav")))).toBe(false);
  });
});

describe("a member through the runtime's own hosting", () => {
  it("says a 409 after a host call and never reruns it", async () => {
    let calls = 0;
    server.serve("peak", () => {
      calls++;
      // The project moves under the run: the set is then refused as stale.
      withVfs(disk, () =>
        writeDoc(join(dir, "project.k7x"), {
          project: { name: "t", tempo: 100 + calls },
        }),
      );
      return { status: 200, body: { db: -3 } };
    });
    const r = await rt.run("rth peak a");
    expect(calls).toBe(1);
    expect(r.committed).toBe(false);
    expect(r.output.join("\n")).toContain("the project moved");
  });

  it("sends the project the runtime opened as `at`", async () => {
    let at: unknown;
    server.serve("peak", (body) => {
      at = body?.at;
      return { status: 200, body: { db: -3 } };
    });
    await rt.run("rth peak a");
    expect(at).toBe("song.k7");
  });

  it("takes turns: a second run starts when the first has ended", async () => {
    trace.length = 0;
    server.serve("peak", async () => {
      await new Promise((ok) => setTimeout(ok, 30));
      return { status: 200, body: { db: -3 } };
    });
    const a = rt.run("rth peak a");
    const b = rt.run("rth peak b");
    await Promise.all([a, b]);
    expect(trace).toEqual(["start a", "end a", "start b", "end b"]);
  });
});
