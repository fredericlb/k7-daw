// runner.test.ts — the BUILT Worker bundle over HTTP (in a worker_threads thread), against a fake server.
// A reader would get wrong that these run the bundles, not the source: the aliases, the wasm chunk and `?raw` are what is proven.

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { gzipSync, strToU8, zipSync } from "fflate";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { compileReady } from "./testing/compileDisk.ts";
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
  await compileReady();
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
  it("reads a project's picture views in the Worker bundle", async () => {
    const w = await startWorker(join(out, "worker"));
    const page = w.page;
    try {
      await page.init(http.base);
      await page.open("song.k7");
      expect(await page.videoErrors()).toEqual({ errors: [], diagnostics: [] });
      expect(
        w.built.files.some((f: { path: string }) =>
          f.path.endsWith("worker.js"),
        ),
      ).toBe(true);
    } finally {
      await w.stop();
    }
  }, 120_000);

  it("imports a zipped Ableton project with samples, in the Worker bundle", async () => {
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
      await h.close();
      rmSync(base, { recursive: true, force: true });
    }
  }, 120_000);
});
