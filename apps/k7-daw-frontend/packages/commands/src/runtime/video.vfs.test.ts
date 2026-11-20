// video.vfs.test.ts — the Worker's two picture views over the corpus' picture projects, and over an edit.
// What a reader gets wrong: the expected answer is the loader run straight over the same disk; the golden corpus already pins the loader itself.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pictureLanes, stripLanes } from "@studio/song/laneStrip";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { ensureVideoRun } from "@studio/song/videoOutputs";
import { loadVideoProject } from "@studio/song-video/videoProject";
import { webVideoProjectOf } from "@studio/song-video/webVideo";
import { afterEach, describe, expect, it } from "vitest";
import { provideCommandsYaml } from "../commandSpec.ts";
import { commandsYamlText, realFs } from "./realFs.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { current, setVfs, withVfs } from "./shims/current.ts";
import { type FakeServer, fakeServer } from "./testing/fakeServer.ts";
import { Vfs } from "./vfs.ts";

const repo = join(import.meta.dirname, "../../../../../..");
const root = "/ws";
const BINARY = /\.(wav|mp4|png|jpg|webm|onnx)$/i;

/** The corpus' picture projects: their folder in the repo. */
const CORPUS = [
  { name: "video-demo.k7", from: "demos" },
  { name: "video-lane.k7", from: "e2e/fixtures" },
] as const;

/** A folder of the repo copied into the Vfs disk; a binary is a stand-in, since only its name and length reach a picture. */
function copyIn(from: string, to: string): void {
  const fs = realFs();
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    const src = join(from, e.name);
    if (e.isDirectory()) copyIn(src, join(to, e.name));
    else {
      current().mkdir(dirname(join(to, e.name)), true);
      writeFileSync(
        join(to, e.name),
        BINARY.test(e.name) ? "x" : fs.readFileSync(src, "utf8"),
      );
    }
  }
}

/** The picture as the server answers it: the loader run straight over the disk the bundle was cut from, after the open repairs. */
function direct(name: string): { doc: unknown; errors: string[] } {
  const dir = join(root, name);
  return withVfs(disk, () => {
    const p = loadVideoProject(dir);
    return {
      doc: webVideoProjectOf(p, (abs) =>
        abs.startsWith(`${root}/`) ? abs.slice(root.length + 1) : abs,
      ),
      errors: p.warnings,
    };
  });
}

let disk: Vfs;
let server: FakeServer;
let rt: Runtime | undefined;

async function open(name: string, from?: string, repair = true): Promise<void> {
  provideCommandsYaml(commandsYamlText());
  disk = new Vfs(true);
  setVfs(disk);
  if (from) {
    copyIn(join(repo, from, name), join(root, name));
    if (realFs().existsSync(join(repo, from, "_modules")))
      copyIn(join(repo, from, "_modules"), join(root, "_modules"));
    // The server opens a project through these repairs, so the bundle the Worker reads has them.
    if (repair)
      withVfs(disk, () => {
        ensureVideoRun(join(root, name));
        stripLanes(join(root, name), pictureLanes);
      });
  }
  server = fakeServer(disk, root, name);
  rt = createRuntime({ remote: server });
  await rt.open(name);
}

afterEach(() => rt?.close());

const edit = (rel: string, change: (text: string) => string): void =>
  withVfs(disk, () => {
    const file = join(root, rel);
    writeFileSync(file, change(readFileSync(file, "utf8")));
  });

describe("the picture views over the corpus", () => {
  for (const p of CORPUS)
    it(`answers ${p.name} as the server's compile did`, async () => {
      await open(p.name, p.from);
      const want = direct(p.name);
      expect(await (rt as Runtime).videoDoc()).toEqual({ doc: want.doc });
      expect(await (rt as Runtime).videoErrors()).toEqual({
        errors: want.errors,
        diagnostics: [],
      });
    }, 60_000);
});

describe("a project with no picture", () => {
  it("answers an empty picture and no errors, never a throw", async () => {
    await open("song.k7");
    withVfs(disk, () => {
      writeDoc(join(root, "song.k7", "project.k7x"), {
        project: { name: "t", tempo: 120 },
      });
      writeDoc(join(root, "song.k7", "graph.k7x"), { modules: {} });
      writeLane(join(root, "song.k7"), "gtr", "track", { events: [] });
    });
    await (rt as Runtime).open("song.k7");
    const r = await (rt as Runtime).videoDoc();
    expect("doc" in r && r.doc.videoTracks).toEqual([]);
    expect(await (rt as Runtime).videoErrors()).toEqual({
      errors: [],
      diagnostics: [],
    });
  });
});

describe("after an edit", () => {
  it("answers the picture a verb just changed", async () => {
    // Unrepaired: the verb's own compile reads no strip, which the Worker's embedded meta has no shader for.
    await open("video-lane.k7", "e2e/fixtures", false);
    const mute = async (): Promise<boolean | undefined> => {
      const r = await (rt as Runtime).videoDoc();
      return "doc" in r
        ? r.doc.videoTracks.find((t) => t.id === "main")?.mute
        : undefined;
    };
    expect(await mute()).toBe(false);
    const ran = await (rt as Runtime).run("picture mute main");
    expect(ran.output.join("")).toContain("hidden");
    expect(ran.committed).toBe(true);
    expect(await mute()).toBe(true);
  }, 60_000);

  it("answers a page reading at a token from before its own edit without opening the project again", async () => {
    await open("video-lane.k7", "e2e/fixtures", false);
    const before = server.token();
    const ran = await (rt as Runtime).run("picture mute main");
    expect(ran.committed).toBe(true);
    const opens = server.calls.bundle;
    // The page's token read raced the edit: an open costs a phone seconds, and the runtime already holds newer files.
    const r = await (rt as Runtime).videoDoc(before);
    expect(server.calls.bundle).toBe(opens);
    expect(
      "doc" in r && r.doc.videoTracks.find((t) => t.id === "main")?.mute,
    ).toBe(true);
  }, 60_000);

  it("answers the picture a peer wrote, once the page names the revision", async () => {
    await open("video-demo.k7", "demos");
    const viewport = async (token?: string): Promise<unknown> => {
      const r = await (rt as Runtime).videoDoc(token);
      return "doc" in r ? r.doc.viewport : r.error;
    };
    expect(await viewport()).toEqual([1280, 720]);
    edit("video-demo.k7/project.k7x", (t) =>
      t.replace("[1280, 720, 30]", "[640, 360, 30]"),
    );
    expect(await viewport(server.token())).toEqual([640, 360]);
  }, 60_000);

  it("says the compile's own sentence for a picture that cannot be built, as an answer", async () => {
    await open("video-demo.k7", "demos");
    edit("video-demo.k7/project.k7x", (t) =>
      t.replace("viewport: [1280, 720, 30],", ""),
    );
    const r = await (rt as Runtime).videoDoc(server.token());
    expect(r).toEqual({ error: expect.stringContaining("viewport") });
    const e = await (rt as Runtime).videoErrors();
    expect(e.errors[0]).toContain("viewport");
  }, 60_000);
});
