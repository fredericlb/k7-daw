// importer.vfs.test.ts — the Worker's import over a fake workspace: samples first, texts in one set, a failed set takes its folder, hostile inputs.
// What a reader gets wrong: the fake's disk is a text disk, so an uploaded sample is a placeholder there and its bytes sit in `blobs`.

import { MAX_INFLATED_BYTES } from "@studio/translator/inflate";
import { gzipSync, strToU8, zipSync } from "fflate";
import { beforeEach, describe, expect, it } from "vitest";
import { createImporter, type Importer } from "./importer.ts";
import { setVfs } from "./shims/current.ts";
import { type FakeWorkspace, fakeWorkspace } from "./testing/fakeWorkspace.ts";
import { Vfs } from "./vfs.ts";

const ROOT = "/ws";
const MiB = 2 ** 20;

/** A set with one audio clip naming `relative`; `entity` adds a doctype that declares one. */
const als = (relative: string, extra = ""): Uint8Array =>
  gzipSync(
    strToU8(`<?xml version="1.0" encoding="UTF-8"?>${extra}
<Ableton MajorVersion="5" MinorVersion="12.0_12402" Creator="Ableton Live 12.0">
  <LiveSet>
    <MainTrack><DeviceChain><Mixer>
      <Tempo><Manual Value="110" /></Tempo>
      <TimeSignature><Manual Value="201" /></TimeSignature>
    </Mixer></DeviceChain></MainTrack>
    <Tracks>
      <AudioTrack Id="1">
        <Name><EffectiveName Value="audio" /></Name>
        <DeviceChain>
          <Mixer><Speaker><Manual Value="true" /></Speaker></Mixer>
          <MainSequencer><Sample><ArrangerAutomation><Events>
            <AudioClip Id="0" Time="0">
              <CurrentStart Value="0" /><CurrentEnd Value="8" />
              <Loop><LoopStart Value="0" /><LoopEnd Value="8" />
                    <StartRelative Value="0" /><LoopOn Value="false" /></Loop>
              <Name Value="take" />
              <SampleRef><FileRef><RelativePath Value="${relative}" />
                <Path Value="C:/elsewhere/take.wav" /><Name Value="take.wav" /></FileRef>
                <DefaultSampleRate Value="44100" /></SampleRef>
              <IsWarped Value="true" />
              <WarpMarkers></WarpMarkers>
            </AudioClip>
          </Events></ArrangerAutomation></Sample></MainSequencer>
        </DeviceChain>
      </AudioTrack>
    </Tracks>
  </LiveSet>
</Ableton>`),
  );

const WAV = new Uint8Array([82, 73, 70, 70, 1, 2, 3]);

/** A zipped Ableton project: the set and the sample it names. */
const projectZip = (): Uint8Array =>
  zipSync({
    "Song Project/Song.als": als("Samples/take.wav"),
    "Song Project/Samples/take.wav": WAV,
  });

let ws: FakeWorkspace;
let importer: Importer;

beforeEach(() => {
  const disk = new Vfs(true);
  setVfs(disk);
  ws = fakeWorkspace(disk, ROOT);
  importer = createImporter({ remote: ws });
});

describe("a dropped zip with samples", () => {
  it("uploads the samples first and lands the texts in one set that creates the folder", async () => {
    const r = await importer.importFile(projectZip(), "Song.zip", "song.k7");
    expect(r.project).toBe("song.k7");
    expect(r.log[0]).toBe("read Song.zip");
    expect(r.log.at(-1)).toBe("wrote song.k7");
    // Every upload came before the one set, and the set names the folder it creates.
    const order = ws.calls.filter(
      (c) => c.startsWith("upload") || c === "changes",
    );
    expect(order.at(-1)).toBe("changes");
    expect(
      order.slice(0, -1).every((c) => c.startsWith("upload song.k7/")),
    ).toBe(true);
    expect(ws.sets).toHaveLength(1);
    expect(ws.sets[0]?.set).toMatchObject({ at: "", create: "song.k7" });
    expect(ws.sets[0]?.status).toBe(200);
    const wav = [...ws.blobs.entries()].find(([k]) => k.endsWith(".wav"));
    expect(wav?.[1]).toEqual(WAV);
    const files = Object.keys(ws.tree("song.k7"));
    expect(files).toContain("song.k7/project.k7x");
    expect(files.some((f) => f.startsWith("song.k7/clips/"))).toBe(true);
  });

  it("reads the engine's plugin list from the plugins member", async () => {
    ws.plugins = [{ name: "Pro-Q 3", format: "VST3", id: "x" }];
    await importer.importFile(projectZip(), "Song.zip", "song.k7");
    expect(ws.calls).not.toContain("remove song.k7");
  });

  it("works with no engine to ask: the member answers 503 and the import goes on", async () => {
    ws.plugins = undefined;
    const r = await importer.importFile(projectZip(), "Song.zip", "song.k7");
    expect(r.project).toBe("song.k7");
  });
});

describe("a set that does not land", () => {
  it("takes the folder and its samples away, and says why", async () => {
    ws.failNextChange(500);
    await expect(
      importer.importFile(projectZip(), "Song.zip", "song.k7"),
    ).rejects.toThrow(/the write was lost/);
    expect(ws.tree("song.k7")).toEqual({});
    expect(
      [...ws.blobs.keys()].filter((k) => k.startsWith("song.k7/")),
    ).toEqual([]);
  });

  it("is told to take the folder when an upload fails before the set", async () => {
    // A name already uploaded makes the second upload of that name a 409.
    const dup = zipSync({
      "A/Song.als": als("Samples/take.wav"),
      "A/Samples/take.wav": WAV,
    });
    const real = ws.upload?.bind(ws);
    let n = 0;
    ws.upload = async (dir, name, bytes) =>
      ++n === 1
        ? { status: 507, body: { error: "disk full" } }
        : (real?.(dir, name, bytes) as never);
    await expect(
      importer.importFile(dup, "Song.zip", "song.k7"),
    ).rejects.toThrow(/disk full/);
    expect(ws.calls).toContain("discard");
    expect(ws.sets).toHaveLength(0);
  });
});

describe("a name", () => {
  it("that a project holds already is refused before anything is uploaded", async () => {
    await importer.importFile(projectZip(), "Song.zip", "song.k7");
    const before = ws.calls.length;
    await expect(
      importer.importFile(projectZip(), "Song.zip", "song.k7"),
    ).rejects.toThrow(/already exists/);
    expect(ws.calls.slice(before)).toEqual([]);
  });

  it("nobody typed is the file's own, made free with a suffix", async () => {
    const a = await importer.importFile(projectZip(), "Song.zip");
    const b = await importer.importFile(projectZip(), "Song.zip");
    expect([a.project, b.project]).toEqual(["Song", "Song-2"]);
  });
});

describe("hostile inputs", () => {
  it("refuses a zip that inflates past the ceiling, having asked the server for nothing", async () => {
    const limit = 4 * 1024;
    const small = createImporter({ remote: ws, inflateLimit: limit });
    const bomb = zipSync({ "Song.als": new Uint8Array(limit + 1) });
    await expect(small.importFile(bomb, "Bomb.zip", "b.k7")).rejects.toThrow(
      /inflates past/,
    );
    expect(ws.calls).toEqual([]);
    // Just under the ceiling the archive passes the inflate check (it then fails as not an .als).
    const under = zipSync({ "Song.als": new Uint8Array(limit) });
    await expect(small.importFile(under, "U.zip", "u.k7")).rejects.not.toThrow(
      /inflates past/,
    );
  });

  it("keeps the production inflate ceiling at 512 MiB", () => {
    expect(MAX_INFLATED_BYTES).toBe(512 * MiB);
  });

  it("keeps a zip entry's path that climbs out of its folder inside the project", async () => {
    const zip = zipSync({
      "P/Song.als": als("../../../escape/take.wav"),
      "escape/take.wav": WAV,
      "../../outside.wav": WAV,
    });
    await importer.importFile(zip, "Song.zip", "song.k7");
    for (const rel of [
      ...ws.blobs.keys(),
      ...(ws.sets[0]?.set.ops.map((o) => o.file) ?? []),
    ])
      expect(rel.split("/").includes(".."), rel).toBe(false);
    expect(
      [...ws.blobs.keys()].every(
        (k) => k.startsWith("song.k7/") || k.startsWith("_modules/"),
      ),
    ).toBe(true);
  });

  it("refuses an .als whose trailer declares more than the ceiling", async () => {
    const lie = gzipSync(strToU8("<Ableton/>"));
    new DataView(lie.buffer).setUint32(lie.length - 4, 600 * MiB, true);
    const zip = zipSync({ "Song.als": lie });
    await expect(importer.importFile(zip, "Lie.zip", "l.k7")).rejects.toThrow(
      /inflates past/,
    );
    expect(ws.calls).toEqual([]);
  });

  it("refuses a set that declares XML entities", async () => {
    const bomb = als(
      "x.wav",
      '<!DOCTYPE Ableton [<!ENTITY a "aaaa"><!ENTITY b "&a;&a;&a;&a;">]>',
    );
    const zip = zipSync({ "Song.als": bomb });
    await expect(importer.importFile(zip, "E.zip", "e.k7")).rejects.toThrow(
      /declares XML entities/,
    );
    expect(ws.calls).toEqual([]);
  });

  it("refuses a file past the import's size before it reads a byte, and an empty one", async () => {
    const huge = { size: 513 * MiB } as unknown as Blob;
    Object.setPrototypeOf(huge, Blob.prototype);
    await expect(importer.importFile(huge, "Huge.zip", "h.k7")).rejects.toThrow(
      /larger than 512 MB/,
    );
    await expect(
      importer.importFile(new Uint8Array(0), "Empty.zip", "e.k7"),
    ).rejects.toThrow(/empty/);
    await expect(
      importer.importFile(new Uint8Array([1]), "Winter.als", "w.k7"),
    ).rejects.toThrow(/must be one of/);
  });
});

describe("a set in the tree", () => {
  it("reads the samples beside it, and names the ones outside the workspace without fetching them", async () => {
    // The workspace holds the set and its sample; the set also names a sample the machine keeps elsewhere.
    const disk = new Vfs(true);
    setVfs(disk);
    ws = fakeWorkspace(disk, ROOT);
    ws.blobs.set("Live/Song.als", als("Samples/take.wav"));
    ws.blobs.set("Live/Samples/take.wav", WAV);
    importer = createImporter({ remote: ws });
    const r = await importer.importPath("Live/Song.als");
    expect(r.project).toBe("Song");
    expect(ws.calls).toContain("bytes Live/Samples/take.wav");
    expect([...ws.blobs.keys()].some((k) => k.startsWith("Song/clips/"))).toBe(
      true,
    );

    ws.blobs.set("Live/Far.als", als("../../../../etc/take.wav"));
    const far = await importer.importPath("Live/Far.als");
    expect(far.log.join("\n")).toMatch(
      /1 sample\(s\) live outside the workspace/,
    );
    expect(ws.calls.filter((c) => c.includes("etc/take.wav"))).toEqual([]);
  });

  it("says a missing file and a format it does not take", async () => {
    await expect(importer.importPath("none.dawproject")).rejects.toThrow(
      "no such file: none.dawproject",
    );
    await expect(importer.importPath("a.k7/project.k7x")).rejects.toThrow(
      'cannot import "a.k7/project.k7x": expected one of .dawproject, .als, .zip',
    );
  });
});
