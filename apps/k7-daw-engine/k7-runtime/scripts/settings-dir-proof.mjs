// settings-dir-proof.mjs — runs the real engine's `--render` with a scratch settings folder and proves it
// wrote Settings.xml (and, for `scan`, exts.json) there, while the user's own Settings.xml and exts.json did not move. See docs/settingsDir.md.
// The one thing a reader gets wrong: another engine running meanwhile moves the real mtimes, so a failure is retried.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const moteur = dirname(dirname(fileURLToPath(import.meta.url)));
const bin =
  process.argv[2] ??
  [
    "build/windows-msvc/RelWithDebInfo/k7-runtime.exe",
    "build/macos-clang/k7-runtime",
    "build/dev-gcc/k7-runtime",
    "build/release-gcc/k7-runtime",
  ]
    .map((p) => join(moteur, p))
    .find(existsSync);
if (!bin) throw new Error("no k7-runtime binary: build it first (npm run build:mac --workspace k7-runtime)");

const real = [
  join(homedir(), "Library", "Studio", "Settings.xml"),
  join(homedir(), ".k7", "exts.json"),
  join(process.env.APPDATA ?? "", "Studio", "Settings.xml"),
].filter((p) => existsSync(p));
const mtimes = () => real.map((p) => statSync(p).mtimeMs);

/** One render under `how` (flag or env); true when the scratch folder got Settings.xml and the real files stood still. */
function attempt(how) {
  const scratch = mkdtempSync(join(tmpdir(), "k7-settings-proof-"));
  try {
    writeFileSync(join(scratch, "doc.json"), JSON.stringify({ tracks: [], bpm: 120 }));
    const args = how === "scan" ? ["scan"] : ["--render", join(scratch, "doc.json"), "--out", join(scratch, "out")];
    const folder = join(scratch, "settings");
    const env = { ...process.env };
    delete env.K7_SETTINGS_DIR;
    if (how === "flag") args.push("--settings-dir", folder);
    else env.K7_SETTINGS_DIR = folder;
    const before = mtimes();
    // An empty document renders nothing audible and exits 1: the engine still opened its settings first.
    try {
      execFileSync(bin, args, { env, stdio: ["ignore", "ignore", "ignore"], timeout: 120_000 });
    } catch (e) {
      if (e.status !== 1) throw e;
    }
    const after = mtimes();
    const wrote = existsSync(join(folder, how.startsWith("scan") ? "exts.json" : "Settings.xml"));
    const still = before.every((t, i) => t === after[i]);
    console.log(`${how}: scratch Settings.xml=${existsSync(join(folder, "Settings.xml"))} exts.json=${existsSync(join(folder, "exts.json"))} wrote=${wrote} real mtimes before=${before} after=${after} unmoved=${still}`);
    return wrote && still;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

let ok = true;
for (const how of ["env", "flag", "scan"]) {
  let passed = false;
  for (let i = 0; i < 3 && !passed; i++) passed = attempt(how);
  ok &&= passed;
}
console.log(ok ? "PASS" : "FAIL");
process.exit(ok ? 0 : 1);
