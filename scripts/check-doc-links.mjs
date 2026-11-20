// check-doc-links.mjs — refuse a page or a comment that names a repo file which is not there.
//
// The method of this repo is that the WHY lives in the prose: a comment points at a `docs/` page,
// and the page points back at the code it is about. A pointer to a file that has been renamed or
// deleted sends the reader into a wall, and nothing else notices — so BOTH directions are read.
//
// It reads what git tracks AND what it merely knows about, and only paths that look like OUR tree.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";

/** What counts as "a path into this repo": `apps/` from the root, `packages/` or a sibling app from a page (../docs/lints.md). */
const REF =
  /(?<![\w.\-/])(?:\.\.\/)*(?:apps|packages)\/[\w./-]*\.\w{1,5}\b|(?<![\w.-])(?:\.\.\/)+k7-daw-[\w-]+\/[\w./-]*\.\w{1,5}\b/g;

/** Paths a page may name although this checkout has not got them — kept short (../docs/lints.md). */
const ALLOW = [
  // A build output, which a checkout has only once it has built.
  /\/(dist|target)\//,
];

/** The pages, and the source whose comments point at them (../docs/lints.md). */
const SCANNED = [
  "*.md",
  "*.ts",
  "*.tsx",
  "*.mts",
  "*.mjs",
  "*.cjs",
  "*.js",
  "*.jsx",
  "*.cpp",
  "*.h",
  "*.hpp",
  "*.kt",
  "*.rs",
  "*.k7x",
  "*.glsl",
  "*.yaml",
  "*.yml",
  "*.sh",
  "*.py",
];

/** PIERRES TOMBALES — prefixes of paths a page names because they are gone (../docs/lints.md). */
const TOMBSTONES = [];

// `--others` too, or a page is unchecked until the day it is committed — which is the day it is
// written, and the day its references are most likely to be wrong. `--exclude-standard` keeps the
// ignored tree out. Found by planting a dead reference in a new page and watching this pass.
const files = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "--", ...SCANNED],
  { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
)
  .split("\n")
  .filter(Boolean)
  // Build and install output, in case an ignore rule ever misses one.
  .filter((f) => !/(^|\/)(node_modules|dist|dev-dist|build|out)\//.test(f))
  // A file deleted in the working tree is still in the index until the commit; it names nothing.
  .filter((f) => existsSync(f));

const dead = [];
let cited = 0;
for (const file of files) {
  const text = readFileSync(file, "utf8");
  const lines = text.split("\n");
  for (const [i, line] of lines.entries()) {
    // A Rust test names its fixtures from the crate's own folder (CARGO_MANIFEST_DIR), not the file's.
    if (line.includes("CARGO_MANIFEST_DIR")) continue;
    for (const hit of line.matchAll(REF)) {
      const ref = hit[0];
      cited += 1;
      if (
        TOMBSTONES.some((p) => ref.replace(/^(\.\.\/)+/, "").startsWith(p)) ||
        ALLOW.some((r) => r.test(ref))
      )
        continue;
      // From the repo root first, then relative to the page (../docs/lints.md).
      if (existsSync(ref) || existsSync(normalize(join(dirname(file), ref))))
        continue;
      dead.push({ file, line: i + 1, ref });
    }
  }
}

// A path git ignores is a build output a checkout has only once built (../docs/lints.md).
const ignored = new Set(
  dead.length
    ? spawnSync("git", ["check-ignore", "--no-index", "--stdin"], {
        input: dead.map((d) => d.ref).join("\n"),
        encoding: "utf8",
      }).stdout.split("\n")
    : [],
);
for (let i = dead.length - 1; i >= 0; i--)
  if (ignored.has(dead[i].ref)) dead.splice(i, 1);

if (dead.length) {
  console.error(`${dead.length} reference(s) name a file that is not here:\n`);
  for (const d of dead) console.error(`  ${d.file}:${d.line} — ${d.ref}`);
  console.error(
    "\nRename it, or say what took its place. A page that points at nothing is worse than no page:\n" +
      "it is read as current.",
  );
  process.exit(1);
}

console.log(
  `${cited} source reference(s) in ${files.length} pages and source files, all present`,
);
