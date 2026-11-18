// `playlist stems` — preparing a whole set. Three things would be wrong silently here: a record
// separated into the wrong folder (a deck opens file paths, so a stem one folder off is a fader
// that does nothing), a re-run costing another hour on thirty records already done, and one
// unreadable record taking the other twenty-nine down with it.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { RunColdStep } from "@studio/audio-passes/coldPass";
import { librariesFixture, writeDoc } from "@studio/song/testSupport";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { runCommand } from "./commandSpec";
import { testHost } from "./testHost.ts";

let root: string; // the workspace: the project, its libraries, `_playlists/` and the records
const logs: string[] = [];
const refusals: string[] = [];
/** What the separator was asked to do, in order — the whole point of injecting it. */
let asked: { source: string; outDir: string; step: string }[] = [];

const put = (at: string, rel: string, body: string): void => {
  const path = join(at, rel);
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, body, "utf8");
};

/** A separator that writes nothing and remembers everything. */
const runColdStep: RunColdStep = async ({ step, source, outDir }) => {
  asked.push({ source, outDir, step: step.step });
  return ["drums", "bass", "other", "vocals"].map((s) =>
    join(outDir, `${s}.wav`),
  );
};

/** `null` means no way to run one; `stemsChain` is injected so no real preferences file decides. */
const host = (cold: RunColdStep | null = runColdStep, stemsChain = "") =>
  testHost({
    songPath: join(root, "song"),
    repoDir: root,
    log: (l: string[]) => logs.push(...l),
    refuse: (l: string[]) => refusals.push(...l),
    runColdStep: cold ?? undefined,
    stemsChain,
  });

/** A record of the workspace, by its workspace path. */
const song = (ref: string): void => put(root, ref, "not really audio");

beforeEach(() => {
  logs.length = 0;
  refusals.length = 0;
  asked = [];
  root = mkdtempSync(join(tmpdir(), "td-plstems-"));

  // A project, because the chain is resolved through its module vocabulary.
  writeDoc(join(root, "song", "project.k7x"), {
    project: { name: "t", tempo: 120 },
  });
  // …and the library holding the separator and a chain that names it.
  const lib = librariesFixture(root);
  const doc = (rel: string, value: Record<string, unknown>) =>
    writeDoc(join(lib, rel), value);
  doc("modules/config.k7x", { namespace: "k7-core" });
  doc("modules/audio/stemsplit/module.k7x", {
    cold: true,
    plugin: { type: "offline", name: "Stem Split" },
    params: { onnx: "htdemucs.onnx" },
  });
  put(lib, "modules/audio/stemsplit/assets/htdemucs.onnx", "not a model");
  doc("chains/stems/graph.k7x", {
    modules: { "@in": ["split"], split: ["@out"] },
  });
  doc("chains/stems/nodes/split.k7x", {
    extends: "@k7-core/audio/stemsplit",
  });
  // …and one that treats rather than separates, for the refusal below.
  doc("modules/audio/eq/module.k7x", {
    plugin: { type: "internal", name: "4Band EQ" },
  });
  doc("chains/warm/graph.k7x", { modules: { "@in": ["hp"], hp: ["@out"] } });
  doc("chains/warm/nodes/hp.k7x", { extends: "@k7-core/audio/eq" });
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

const setList = (...refs: string[]): void =>
  writeDoc(join(root, "_playlists", "ete.k7x"), { title: "ete", items: refs });

describe("playlist stems", () => {
  it("separates every record of the set list, into `stems/` beside each", async () => {
    setList("records/hells-bells/hells.wav", "records/lithium/lithium.flac");
    song("records/hells-bells/hells.wav");
    song("records/lithium/lithium.flac");
    await runCommand("playlist stems ete @k7-core/stems", host(), "cli");
    expect(asked.map((a) => a.source)).toEqual([
      join(root, "records", "hells-bells", "hells.wav"),
      join(root, "records", "lithium", "lithium.flac"),
    ]);
    // Under `stems/`: a stem alongside the record could be named by the set list as a record.
    expect(asked[0].outDir).toBe(join(root, "records", "hells-bells", "stems"));
    expect(logs.join(" ")).toMatch(/2 separated/);
  });

  it("SKIPS a song whose stems are already there, and says how many", async () => {
    // Re-running a half-finished preparation must cost the songs that are left and nothing else.
    setList("records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    put(root, "records/lithium/stems/bass.wav", "done");
    await runCommand("playlist stems ete @k7-core/stems", host(), "cli");
    expect(asked).toEqual([]);
    expect(logs.join(" ")).toMatch(/0 separated, 1 already done/);
  });

  it("names a song this machine has not got, and goes on to the next", async () => {
    setList("records/nowhere.wav", "records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    await runCommand("playlist stems ete @k7-core/stems", host(), "cli");
    expect(asked).toHaveLength(1);
    expect(logs.join(" ")).toMatch(
      /records\/nowhere.wav: not in this workspace/,
    );
  });

  it("names a line that is no audio file, or leaves the workspace, rather than passing it on", async () => {
    setList("notes.txt", "../outside.wav");
    put(root, "notes.txt", "x");
    await runCommand("playlist stems ete @k7-core/stems", host(), "cli");
    expect(asked).toEqual([]);
    expect(logs.join(" ")).toMatch(/notes.txt: not in this workspace/);
    expect(logs.join(" ")).toMatch(/\.\.\/outside.wav: not in this workspace/);
  });

  it("refuses a chain that TREATS rather than separates, before touching anything", async () => {
    setList("records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    await runCommand("playlist stems ete @k7-core/warm", host(), "cli");
    expect(refusals.join(" ")).toMatch(/does not separate/);
    expect(asked).toEqual([]);
  });

  it("refuses when the host cannot run an offline step", async () => {
    setList("records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    await runCommand("playlist stems ete @k7-core/stems", host(null), "cli");
    expect(refusals.join(" ")).toMatch(/cannot run an offline step/);
  });

  it("falls back to the chain THIS MACHINE remembers when the line names none", async () => {
    // The point of `system stems`: the same answer every time, so the line does not carry it.
    setList("records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    await runCommand(
      "playlist stems ete",
      host(runColdStep, "@k7-core/stems"),
      "cli",
    );
    expect(asked).toHaveLength(1);
    expect(refusals).toEqual([]);
  });

  it("prefers the chain the LINE names over the machine's", async () => {
    // The override is what keeps the preference from being a ceiling: one run, another model.
    setList("records/lithium/lithium.flac");
    song("records/lithium/lithium.flac");
    await runCommand(
      "playlist stems ete @k7-core/warm",
      host(runColdStep, "@k7-core/stems"),
      "cli",
    );
    expect(refusals.join(" ")).toMatch(/does not separate/);
    expect(asked).toEqual([]);
  });

  it("says what is wrong with the line rather than guessing", async () => {
    // Nothing on the line and nothing remembered: named as the thing to set, not as a usage error.
    await runCommand("playlist stems ete", host(), "cli");
    expect(refusals.join(" ")).toMatch(/system stems/);
    await runCommand("playlist stems", host(), "cli");
    expect(refusals.join(" ")).toMatch(/usage: playlist stems/);
    setList();
    await runCommand("playlist stems ete @k7-core/stems", host(), "cli");
    expect(refusals.join(" ")).toMatch(/names no song/);
  });
});

// Refusals only, and deliberately: succeeding writes the real `~/.k7/preferences.k7x`.
describe("system stems", () => {
  it("refuses a chain that does not separate, and says what it wants", async () => {
    await runCommand("system stems @k7-core/warm", host(), "cli");
    expect(refusals.join(" ")).toMatch(/does not separate/);
  });

  it("refuses a chain that is not there, in the resolver's own words", async () => {
    await runCommand("system stems @k7-core/nope", host(), "cli");
    expect(refusals.join(" ")).toMatch(/no chain/);
  });

  it("with no ref, says what is set rather than assuming", async () => {
    await runCommand(
      "system stems",
      host(runColdStep, "@k7-core/stems"),
      "cli",
    );
    expect(refusals.join(" ")).toMatch(/currently: @k7-core\/stems/);
    refusals.length = 0;
    await runCommand("system stems", host(), "cli");
    expect(refusals.join(" ")).toMatch(/nothing is set/);
  });
});
