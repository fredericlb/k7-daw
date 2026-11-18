import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { stringifyLibraryPins } from "@studio/song-schema/libraryVersions";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { projectRoots } from "./confine";
import { loadPreferences, normalizePreferences } from "./preferences";
import { setLibraryOverlays } from "./sharedFolders";
import { songRig, songRoots } from "./songRoots";

// The preferences file is read at a path fixed when the module loads, so the rig test supplies what it reads.
vi.mock("./preferences", async (original) => ({
  ...(await original<typeof import("./preferences")>()),
  loadPreferences: vi.fn(),
}));

// The crate reads under the roots it is given: what `projectRoots` admits must all be among them (decision 736).
const within = (root: string, p: string) =>
  p === root || p.startsWith(root.endsWith("/") ? root : `${root}/`);

let base: string;
let home: string;
let libs: string;
let ws: string;
let project: string;
const saved = { ...process.env };
const platform = Object.getOwnPropertyDescriptor(process, "platform");

/** Every folder the roots mount, the scan's file left out. */
function mounted(r: ReturnType<typeof songRoots>): string[] {
  return [
    r.project,
    r.workspace,
    r.confine,
    r.meta,
    r.renders,
    r.models,
    ...r.libraries.map((l) => l.root),
    ...r.also_readable,
  ];
}

beforeEach(() => {
  base = realpathSync(mkdtempSync(join(tmpdir(), "songroots-")));
  home = join(base, "alice");
  libs = join(base, "libraries");
  ws = join(home, "workspace", "main");
  project = join(ws, "song");
  mkdirSync(project, { recursive: true });
  mkdirSync(join(libs, "k7-core", "modules"), { recursive: true });
  process.env.STUDIO_HOME = home;
  process.env.STUDIO_LIBRARIES_HOME = libs;
  delete process.env.STUDIO_MODELS;
  delete process.env.DIRT_SAMPLES;
  delete process.env.STRUDEL_SAMPLES;
  setLibraryOverlays([]);
});

afterEach(() => {
  if (platform) Object.defineProperty(process, "platform", platform);
  for (const k of Object.keys(process.env))
    if (!(k in saved)) delete process.env[k];
  Object.assign(process.env, saved);
  setLibraryOverlays([]);
  rmSync(base, { recursive: true, force: true });
});

describe("songRoots", () => {
  it("mounts exactly what projectRoots admits, none less and none more", () => {
    mkdirSync(join(home, "library"), { recursive: true });
    const r = songRoots(project);
    const allowed = projectRoots(project);
    // Every mount lies in an allowed root, and every allowed root in a mount.
    for (const m of mounted(r))
      expect(allowed.some((a) => within(a, m))).toBe(true);
    for (const a of allowed)
      expect(mounted(r).some((m) => within(m, a))).toBe(true);
    expect(r.project).toBe(project);
    expect(r.workspace).toBe(ws);
    expect(r.confine).toBe(ws);
    expect(r.scan).toBe(join(home, "exts.json"));
    expect(r.renders).toBe(join(home, ".renders"));
    expect(r.paths).toBe("posix");
    expect(r.workspace_namespace).toBeUndefined();
  });

  it("lists the account's library first, editable, then every installed one by name", () => {
    mkdirSync(join(libs, "drums"), { recursive: true });
    const r = songRoots(project);
    expect(r.libraries.map((l) => [l.name, l.editable])).toEqual([
      ["account", true],
      ["drums", false],
      ["k7-core", false],
    ]);
    expect(r.libraries[0].root).toBe(join(home, "library"));
  });

  it("reads a library at the version the workspace pins, and admits its other versions", () => {
    for (const v of ["1", "2", "draft"])
      mkdirSync(join(libs, "drums", v), { recursive: true });
    writeFileSync(
      join(ws, "_library-pins.k7x"),
      stringifyLibraryPins({ drums: 2 }),
    );
    const r = songRoots(project);
    expect(r.libraries.find((l) => l.name === "drums")?.root).toBe(
      join(libs, "drums", "2"),
    );
    // The version folders not pinned stay readable, as the TypeScript admits them.
    expect(r.also_readable).toContain(join(libs, "drums"));
    expect(
      projectRoots(project).some((a) => within(a, join(libs, "drums", "1"))),
    ).toBe(true);
  });

  it("marks a library whose overlay is the folder read as editable", () => {
    const overlay = join(base, "draft-drums");
    mkdirSync(overlay, { recursive: true });
    setLibraryOverlays([{ name: "drums", root: overlay }]);
    const r = songRoots(project);
    expect(r.libraries.find((l) => l.name === "drums")).toEqual({
      name: "drums",
      root: overlay,
      editable: true,
    });
    expect(r.also_readable).toContain(overlay);
  });

  it("names the library a project inside it belongs to as the level of its _modules", () => {
    const inside = join(libs, "k7-core", "proj");
    mkdirSync(inside, { recursive: true });
    const r = songRoots(inside);
    expect(r.workspace).toBe(join(libs, "k7-core"));
    expect(r.workspace_namespace).toBe("k7-core");
  });

  it("states the platform's rule and spells a Windows path with a slash", () => {
    Object.defineProperty(process, "platform", { value: "win32" });
    process.env.STUDIO_MODELS = "models\\cache";
    const r = songRoots(project);
    expect(r.paths).toBe("windows");
    expect(r.models.endsWith("/cache")).toBe(true);
    expect(JSON.stringify(r)).not.toContain("\\\\");
  });

  // Decision 737: a family folder that is a link out is readable at its real path.
  it("adds the real path of a library's family folder that is a link out", () => {
    const outside = join(base, "repo-samples");
    mkdirSync(outside, { recursive: true });
    symlinkSync(outside, join(libs, "k7-core", "samples"));
    const r = songRoots(project);
    expect(r.also_readable).toContain(outside);
    // A family that is a plain folder adds nothing of its own.
    expect(r.also_readable).not.toContain(join(libs, "k7-core", "modules"));
  });

  // Decision 741: songRoots has no memo, so a pins file that stops reading costs the pins on the very next call.
  it("costs the pins and nothing else when the pins file does not read", () => {
    mkdirSync(join(libs, "drums", "1"), { recursive: true });
    mkdirSync(join(libs, "drums", "2"), { recursive: true });
    const pins = join(ws, "_library-pins.k7x");
    writeFileSync(pins, stringifyLibraryPins({ drums: 2 }));
    chmodSync(pins, 0o000);
    let readable = true;
    try {
      readFileSync(pins);
    } catch {
      readable = false;
    }
    const r = songRoots(project);
    if (readable) {
      // A user who can read a mode-000 file (root) gets the pin: the case cannot run here.
      expect(r.libraries.find((l) => l.name === "drums")?.root).toBe(
        join(libs, "drums", "2"),
      );
    } else {
      expect(r.libraries.find((l) => l.name === "drums")?.root).toBe(
        join(libs, "drums", "1"),
      );
    }
    expect(r.project).toBe(project);
    expect(r.libraries.map((l) => l.name)).toContain("k7-core");
    chmodSync(pins, 0o644);
    writeFileSync(pins, "{{ not a pin file");
    expect(
      songRoots(project).libraries.find((l) => l.name === "drums")?.root,
    ).toBe(join(libs, "drums", "1"));
  });

  it("costs nothing when the libraries' home is missing", () => {
    rmSync(libs, { recursive: true, force: true });
    const r = songRoots(project);
    expect(r.libraries.map((l) => l.name)).toEqual(["account"]);
  });

  it("names the sample banks the machine sets", () => {
    process.env.DIRT_SAMPLES = join(base, "dirt");
    expect(songRoots(project).sample_banks).toEqual([join(base, "dirt")]);
  });
});

describe("songRig", () => {
  it("is the channels and the cue pair of the preferences, normalized", () => {
    vi.mocked(loadPreferences).mockReturnValue(
      normalizePreferences({
        channels: {
          in: [
            { ch: [1, 2], label: "Mic" },
            { ch: 0, label: "Dead" },
          ],
        },
        cueOutput: " Cue ",
      }),
    );
    expect(songRig()).toEqual({
      channels: { in: [{ ch: [1, 2], label: "Mic" }], out: [] },
      cueOutput: "Cue",
    });
  });

  it("is the empty rig when the preferences do not read", () => {
    vi.mocked(loadPreferences).mockReturnValue(
      normalizePreferences("not a map"),
    );
    expect(songRig()).toEqual({ channels: { in: [], out: [] } });
  });
});
