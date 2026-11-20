// preferences.test.ts — the machine's preferences file: k7x on disk, a YAML of before is left for the Rust server's upgrade.
// Each test gets a studio home of its own (a temp folder with a space and an accent), never the real one.

import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parse } from "@studio/k7x/document";
import { stringify } from "@studio/k7x/print";
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

type Mod = typeof import("./preferences");

const full = {
  audioDevice: { name: "Focusrite: USB #2 (ASIO)", type: "ASIO" },
  channels: {
    in: [
      { ch: [1], label: "guitare électrique" },
      { ch: [3, 4], label: "123" },
      { ch: [5], label: "yes" },
      { ch: [6], label: "a: b # c" },
    ],
    out: [{ ch: [1, 2], label: "écoute – monitors" }],
  },
  inputPairs: [1, 3, 9],
  editor: { vim: false },
  stemsChain: "séparer: stems #1",
  instrumentalChain: "no voice",
  cueOutput: [3, 4],
};

let home = "";
let prev: string | undefined;
let m: Mod;
const k7x = () => join(home, "preferences.k7x");
const yaml = () => join(home, "preferences.yaml");
const files = () => readdirSync(home).sort();

const YAML_FULL = `audioDevice:
  name: "Focusrite: USB #2 (ASIO)"
  type: ASIO
channels:
  in:
    - { ch: [1], label: guitare électrique }
    - { ch: [3, 4], label: "123" }
  out:
    - { ch: [1, 2], label: "écoute – monitors" }
inputPairs: [2, 3]
editor: { vim: false }
stemsChain: " séparer " 
cueOutput: monitors
`;

// The first import transforms the module graph cold, which starves under load; pay it once, here.
beforeAll(async () => {
  await import("./preferences");
}, 120_000);

beforeEach(async () => {
  prev = process.env.STUDIO_HOME;
  home = mkdtempSync(join(tmpdir(), "k7 prefs é-"));
  process.env.STUDIO_HOME = home;
  vi.resetModules();
  m = await import("./preferences");
});
afterEach(() => {
  if (prev === undefined) delete process.env.STUDIO_HOME;
  else process.env.STUDIO_HOME = prev;
  try {
    chmodSync(home, 0o755);
  } catch {
    // already gone
  }
  rmSync(home, { recursive: true, force: true });
});

describe("the file's name and its round trip (1100)", () => {
  it("lives at <studio>/preferences.k7x, under a path with a space and an accent", () => {
    expect(m.PREFS_PATH).toBe(k7x());
  });

  it("writes then reads every field back exactly", () => {
    const p = m.normalizePreferences(full);
    expect(m.parsePreferences(m.stringifyPreferences(p))).toEqual(p);
    m.savePreferences(p);
    expect(m.loadPreferences()).toEqual(p);
    expect(p.audioDevice?.name).toBe("Focusrite: USB #2 (ASIO)");
    expect(p.channels.in[1].label).toBe("123");
    expect(p.channels.in[2].label).toBe("yes");
  });

  it("round-trips the empty and the default preferences, and empty strings the rig drops", () => {
    for (const raw of [{}, null, { stemsChain: "", cueOutput: [] }]) {
      const p = m.normalizePreferences(raw);
      expect(m.parsePreferences(m.stringifyPreferences(p))).toEqual(p);
    }
  });

  it("keeps an unknown key through k7x itself, and drops it where every save already did", () => {
    const raw = { ...full, futureField: { a: [1, "é"] } };
    expect(parse(stringify(raw))).toEqual(raw);
    expect(m.normalizePreferences(raw)).not.toHaveProperty("futureField");
  });

  it("writes no temp file beside it, and replaces the file whole", () => {
    m.savePreferences(m.normalizePreferences(full));
    m.savePreferences(m.normalizePreferences({ editor: { vim: true } }));
    expect(files()).toEqual(["preferences.k7x"]);
    expect(m.loadPreferences().editor.vim).toBe(true);
  });
});

describe("a malformed or unwritable k7x", () => {
  it("reads as the defaults, stays as it is, and is not replaced by the YAML beside it", () => {
    const bad = "editor: {{{ nope";
    writeFileSync(k7x(), bad, "utf8");
    writeFileSync(yaml(), YAML_FULL, "utf8");
    expect(m.loadPreferences()).toEqual(m.defaultPreferences());
    expect(readFileSync(k7x(), "utf8")).toBe(bad);
    expect(readFileSync(yaml(), "utf8")).toBe(YAML_FULL);
  });

  it.skipIf(process.platform === "win32" || process.getuid?.() === 0)(
    "keeps the good file whole when a save cannot write, leaving no temp",
    () => {
      m.savePreferences(m.normalizePreferences(full));
      const before = readFileSync(k7x(), "utf8");
      chmodSync(home, 0o555);
      expect(() => m.savePreferences(m.defaultPreferences())).toThrow();
      expect(readFileSync(k7x(), "utf8")).toBe(before);
      expect(files()).toEqual(["preferences.k7x"]);
    },
  );

  it("creates the studio home when it is not there yet", () => {
    rmSync(home, { recursive: true });
    m.savePreferences(m.normalizePreferences(full));
    expect(m.loadPreferences()).toEqual(m.normalizePreferences(full));
    mkdirSync(home, { recursive: true });
  });
});
