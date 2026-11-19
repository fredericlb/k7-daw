// macSign.mjs — signs every Mach-O the staged resources hold, inside-out, before `tauri build` signs the app: Tauri
// signs only the main binary, and notarization refuses any unsigned code under Contents/Resources (docs/macos.md).
// Identity `$APPLE_SIGNING_IDENTITY`, or "-" (ad-hoc: no timestamp, runs on this Mac only).

import { execFileSync } from "node:child_process";
import { closeSync, openSync, readdirSync, readSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const macos = join(here, "..", "src-tauri", "macos");

/** The entitlements a binary runs with, by its name; none means the hardened runtime alone. */
// `node` is signed only while the bundle stages one: the walk finds what is there.
const ENTITLEMENTS = {
  node: "node.entitlements",
  "k7-runtime": "engine.entitlements",
};

/** Mach-O magics (thin and fat, both byte orders): the first four bytes name a binary, whatever its extension. */
const MAGICS = new Set([
  0xfeedface, 0xfeedfacf, 0xcefaedfe, 0xcffaedfe, 0xcafebabe, 0xbebafeca,
]);

function isMachO(path) {
  const fd = openSync(path, "r");
  try {
    const head = Buffer.alloc(4);
    return (
      readSync(fd, head, 0, 4, 0) === 4 && MAGICS.has(head.readUInt32BE(0))
    );
  } finally {
    closeSync(fd);
  }
}

/** Every regular file under `dir`. */
function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return walk(p);
    return e.isFile() ? [p] : [];
  });
}

/** The libraries first, the programs after: a program's seal covers the libraries it carries. */
export function order(files) {
  const program = (f) =>
    basename(f) in ENTITLEMENTS || !/\.(dylib|node|so)$/.test(f);
  return [...files.filter((f) => !program(f)), ...files.filter(program)];
}

/** The `codesign` arguments for one binary. */
export function codesignArgs(file, identity) {
  const adHoc = identity === "-";
  const args = ["--force", "--sign", identity, "--options", "runtime"];
  args.push(adHoc ? "--timestamp=none" : "--timestamp");
  const ent = ENTITLEMENTS[basename(file)];
  if (ent) args.push("--entitlements", join(macos, ent));
  return [...args, file];
}

export function signStaged(
  dir,
  identity = process.env.APPLE_SIGNING_IDENTITY || "-",
) {
  const files = order(
    walk(dir).filter((f) => statSync(f).size > 0 && isMachO(f)),
  );
  for (const f of files)
    execFileSync("codesign", codesignArgs(f, identity), { stdio: "inherit" });
  return files.length;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dir = process.argv[2];
  if (!dir) throw new Error("usage: node macSign.mjs <staged resources dir>");
  console.log(`signed ${signStaged(dir)} binaries`);
}
