// runner.test.ts — the two BUILT artifacts over HTTP: the node runner and the Worker (in a worker_threads thread), against a fake server.
// A reader would get wrong that these run the bundles, not the source: the aliases, the wasm chunk and `?raw` are what is proven.

import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { compileNodeProject } from "@studio/song/nodeProject";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { wsWriteData } from "@studio/song/yaml/workspaceDoc";
import { gzipSync, strToU8, zipSync } from "fflate";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../scripts/bundleRuntime.mjs";
import { fakeServer } from "./testing/fakeServer.ts";
import { fakeWorkspace } from "./testing/fakeWorkspace.ts";
import { serveHttp } from "./testing/httpServer.ts";
import { startWorker } from "./testing/workerHarness.ts";
import { Vfs } from "./vfs.ts";

const AMP = "@workspace/audio/amp";

/** A 48 kHz stereo 16-bit wav of `seconds` of silence. */
function silentWav(seconds: number): Buffer {
  const bytes = Math.round(seconds * 48000) * 4;
  const b = Buffer.alloc(44 + bytes);
  b.write("RIFF", 0);
  b.writeUInt32LE(36 + bytes, 4);
  b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(2, 22);
  b.writeUInt32LE(48000, 24);
  b.writeUInt32LE(192000, 28);
  b.writeUInt16LE(4, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36);
  b.writeUInt32LE(bytes, 40);
  return b;
}
let root: string;
let out: string;
let http: Awaited<ReturnType<typeof serveHttp>>;

beforeAll(async () => {
  root = mkdtempSync(join(tmpdir(), "rt-http-"));
  out = mkdtempSync(join(tmpdir(), "rt-out-"));
  const dir = join(root, "song.k7");
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  writeDoc(join(dir, "graph.k7x"), { modules: { gtr: [AMP], [AMP]: ["OUT"] } });
  writeLane(dir, "gtr", "track", { events: [] });
  writeDoc(join(root, "_modules", "audio", "amp", "module.k7x"), {
    plugin: { type: "internal", name: "Chorus" },
  });
  http = await serveHttp(fakeServer(new Vfs(true), root, "song.k7"));
}, 60_000);

afterAll(async () => {
  await http.close();
  rmSync(root, { recursive: true, force: true });
  rmSync(out, { recursive: true, force: true });
});

describe("the built runtime", () => {
  it("runs a verb in node, through the bundle with the shims", async () => {
    const dist = join(out, "node");
    await bundleRuntime({ target: "node", outdir: dist });
    const { createNodeRunner } = await import(
      pathToFileURL(join(dist, "node.mjs")).href
    );
    const rt = createNodeRunner({ base: http.base });
    await rt.open("song.k7");
    const r = await rt.run("node new Audio/AMP hall");
    expect(r.committed).toBe(true);
    expect(existsSync(join(root, "song.k7", "nodes", "hall.k7x"))).toBe(true);
    expect(readFileSync(join(root, "song.k7", "graph.k7x"), "utf8")).toContain(
      "hall",
    );
    rt.close();
  }, 60_000);

  it("runs a verb in the Worker bundle, with the compile loaded as a chunk", async () => {
    const w = await startWorker(join(out, "worker"));
    const page = w.page;
    try {
      await page.init(http.base);
      await page.open("song.k7");
      expect(
        (await page.catalogue()).some((c) => c.command === "node new"),
      ).toBe(true);
      expect((await page.complete("track rename g")).length).toBeGreaterThan(0);
      const r = await page.run("node new Audio/AMP pads");
      expect(r.committed).toBe(true);
      expect(existsSync(join(root, "song.k7", "nodes", "pads.k7x"))).toBe(true);
      expect(
        w.built.files.some((f: { path: string }) =>
          f.path.endsWith("worker.js"),
        ),
      ).toBe(true);
    } finally {
      await w.stop();
    }
  }, 120_000);

  it("consolidates a selection in the Worker through the host's render and adopts the file", async () => {
    const demo = join(
      dirname(fileURLToPath(import.meta.url)),
      "..",
      "..",
      "..",
      "..",
      "demos",
      "node-demo.k7",
    );
    const base = mkdtempSync(join(tmpdir(), "rt-cons-"));
    cpSync(demo, join(base, "song.k7"), { recursive: true });
    mkdirSync(join(base, "_modules", "transformers"), { recursive: true });
    const track = compileNodeProject(join(base, "song.k7")).tracks.find((t) =>
      t.clips.some((c) => c.type === "audio"),
    );
    const fake = fakeServer(new Vfs(true), base, "song.k7");
    const job = fake.startJob([
      {
        state: "done",
        value: {
          manifest: { mix: "mix.wav", stems: [] },
          files: [{ out: "mix.wav", len: 4, seconds: 3.5 }],
        },
      },
    ]);
    fake.stage(job, "mix.wav", "RIFF");
    let asked = 0;
    fake.serve("render", () => {
      asked++;
      return { status: 202, body: { job } };
    });
    const h = await serveHttp(fake);
    const w = await startWorker(join(out, "worker-cons"));
    try {
      await w.page.init(h.base);
      await w.page.open("song.k7");
      const r = await w.page.run(`selection consolidate ${track?.id} 1:1 9:1`);
      expect(r.output.join("\n")).toMatch(/3\.5\d*s rendered/);
      expect(r.committed).toBe(true);
      expect(asked).toBe(1);
      expect(fake.sets[0]?.set.ops.some((o) => o.kind === "adopt")).toBe(true);
    } finally {
      await w.stop();
      await h.close();
      rmSync(base, { recursive: true, force: true });
    }
  }, 120_000);

  it("processes a clip through a chain of the project in the Worker, renders it by the host and repoints the clip", async () => {
    const base = mkdtempSync(join(tmpdir(), "rt-proc-"));
    const dir = join(base, "song.k7");
    const put = (root: string, rel: string, value: Record<string, unknown>) => {
      mkdirSync(dirname(join(root, rel)), { recursive: true });
      wsWriteData(join(root, rel), value);
    };
    put(dir, "project.k7x", {
      project: { name: "t", tempo: 120, sampleRate: 48000 },
    });
    put(dir, "graph.k7x", {
      modules: { gtr: ["amp"], amp: ["strip"], strip: ["OUT"] },
    });
    mkdirSync(join(dir, "clips"), { recursive: true });
    writeFileSync(join(dir, "clips", "riff.wav"), silentWav(0.5));
    put(dir, "clips/riff.k7x", { source: "clips/riff.wav" });
    put(dir, "nodes/gtr.k7x", {
      extends: "@meta/track",
      events: [{ ref: "riff", start: "1:1" }],
    });
    put(base, "_modules/audio/amp/module.k7x", {
      plugin: { type: "internal", name: "Chorus" },
      params: { Depth: 3 },
    });
    put(dir, "nodes/amp.k7x", { extends: "@workspace/audio/amp" });
    put(base, "_modules/audio/strip/module.k7x", {
      plugin: { type: "internal", name: "Mixer" },
      params: { Volume: -6 },
    });
    put(dir, "nodes/strip.k7x", { extends: "@workspace/audio/strip" });
    const fake = fakeServer(new Vfs(true), base, "song.k7");
    const job = fake.startJob([
      {
        state: "done",
        value: {
          manifest: { mix: "mix.wav", stems: [] },
          files: [{ out: "mix.wav", len: 4, seconds: 0.5 }],
        },
      },
    ]);
    fake.stage(job, "mix.wav", "RIFF");
    let asked = 0;
    fake.serve("render", () => {
      asked++;
      return { status: 202, body: { job } };
    });
    const h = await serveHttp(fake);
    const w = await startWorker(join(out, "worker-proc"));
    try {
      await w.page.init(h.base);
      await w.page.open("song.k7");
      const r = await w.page.run("clip process riff gtr");
      expect(r.output.join("\n")).not.toMatch(
        /could not|no chain|needs the server/,
      );
      expect(r.committed, r.output.join("\n")).toBe(true);
      expect(asked).toBe(1);
      const ops = fake.sets[0]?.set.ops ?? [];
      expect(ops.some((o) => o.kind === "adopt")).toBe(true);
      const clip = ops.find(
        (o) => o.kind === "text" && o.file.endsWith("riff.k7x"),
      );
      expect(clip).toMatchObject({
        text: expect.stringMatching(/riff_gtr_\d+T\d+\.wav/),
      });
    } finally {
      await w.stop();
      await h.close();
      rmSync(base, { recursive: true, force: true });
    }
  }, 120_000);

  it("saves the bench's chain from the Worker by the project's name, which is all the route resolves", async () => {
    const base = mkdtempSync(join(tmpdir(), "rt-bench-"));
    cpSync(
      join(
        dirname(fileURLToPath(import.meta.url)),
        "..",
        "..",
        "..",
        "..",
        "demos",
        "node-demo.k7",
      ),
      join(base, "song.k7"),
      { recursive: true },
    );
    const fake = fakeServer(new Vfs(true), base, "song.k7");
    const seen: unknown[] = [];
    // The route's rule: a folder is never the bench; the project's name is.
    fake.serve("/bench/save", (body) => {
      seen.push(body);
      return body?.dir !== undefined || body?.at !== "song.k7"
        ? { status: 400, body: { error: "only the bench saves a chain" } }
        : {
            status: 200,
            body: { ref: "@account/mine", steps: 1, outside: [] },
          };
    });
    const h = await serveHttp(fake);
    const w = await startWorker(join(out, "worker-bench"));
    try {
      await w.page.init(h.base);
      await w.page.open("song.k7");
      const r = await w.page.run("chain save mine");
      expect(r.output.join("\n")).toContain("saved @account/mine");
      expect(seen).toEqual([{ name: "mine", at: "song.k7" }]);
    } finally {
      await w.stop();
      await h.close();
      rmSync(base, { recursive: true, force: true });
    }
  }, 120_000);

  it("imports a zipped Ableton project with samples, in the Worker bundle and in the node runner", async () => {
    const base = mkdtempSync(join(tmpdir(), "rt-import-"));
    const xml = `<?xml version="1.0"?><Ableton MajorVersion="5" MinorVersion="12.0_12402" Creator="Ableton Live 12.0"><LiveSet>
      <MainTrack><DeviceChain><Mixer><Tempo><Manual Value="110" /></Tempo><TimeSignature><Manual Value="201" /></TimeSignature></Mixer></DeviceChain></MainTrack>
      <Tracks><AudioTrack Id="1"><Name><EffectiveName Value="audio" /></Name><DeviceChain><Mixer><Speaker><Manual Value="true" /></Speaker></Mixer>
      <MainSequencer><Sample><ArrangerAutomation><Events><AudioClip Id="0" Time="0"><CurrentStart Value="0" /><CurrentEnd Value="8" />
      <Loop><LoopStart Value="0" /><LoopEnd Value="8" /><StartRelative Value="0" /><LoopOn Value="false" /></Loop><Name Value="take" />
      <SampleRef><FileRef><RelativePath Value="Samples/take.wav" /><Path Value="C:/x/take.wav" /><Name Value="take.wav" /></FileRef><DefaultSampleRate Value="44100" /></SampleRef>
      <IsWarped Value="true" /><WarpMarkers></WarpMarkers></AudioClip></Events></ArrangerAutomation></Sample></MainSequencer></DeviceChain></AudioTrack></Tracks></LiveSet></Ableton>`;
    const wav = silentWav(0.25);
    const zip = zipSync({
      "P/Song.als": gzipSync(strToU8(xml)),
      "P/Samples/take.wav": new Uint8Array(wav),
    });
    const fake = fakeWorkspace(new Vfs(true), base);
    const h = await serveHttp(fake);
    const w = await startWorker(join(out, "worker-import"));
    try {
      await w.page.init(h.base);
      const r = await w.page.importFile(new Blob([zip]), "Song.zip", "song.k7");
      expect(r.project).toBe("song.k7");
      expect(r.log.at(-1)).toBe("wrote song.k7");
      expect(existsSync(join(base, "song.k7", "project.k7x"))).toBe(true);
      const sample = [...fake.blobs.entries()].find(([k]) =>
        k.endsWith(".wav"),
      );
      expect(sample?.[0].startsWith("song.k7/clips/")).toBe(true);
      expect(Buffer.from(sample?.[1] ?? []).equals(wav)).toBe(true);
      expect(fake.sets).toHaveLength(1);
      // A taken name is refused by the Worker's own listing, before a byte goes up.
      const asked = fake.calls.length;
      await expect(
        w.page.importFile(new Blob([zip]), "Song.zip", "song.k7"),
      ).rejects.toThrow(/already exists/);
      expect(fake.calls.length).toBe(asked);
    } finally {
      await w.stop();
    }
    try {
      const dist = join(out, "node-import");
      await bundleRuntime({ target: "node", outdir: dist });
      const { createNodeImporter } = await import(
        pathToFileURL(join(dist, "node.mjs")).href
      );
      const imp = createNodeImporter({ base: h.base });
      const r = await imp.importFile(new Blob([zip]), "Song.zip", "node.k7");
      expect(r.project).toBe("node.k7");
      expect(existsSync(join(base, "node.k7", "project.k7x"))).toBe(true);
    } finally {
      await h.close();
      rmSync(base, { recursive: true, force: true });
    }
  }, 120_000);
});
