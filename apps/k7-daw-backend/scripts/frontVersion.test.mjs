// frontVersion.test.mjs — the front's version is the app's, which is the repository's.
// Run by the desktop app's workspace, which stages from this folder (apps/k7-desktop/package.json).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);
const read = (rel) => readFileSync(join(repo, rel), "utf8");
const version = (rel) => JSON.parse(read(rel)).version;

test("the front's version is the app's, which is the repository's", () => {
  const build = read("apps/k7/backend-rs/crates/k7-front/build.rs");
  assert.match(build, /"\.\.\/\.\.\/\.\.\/package\.json"/);
  assert.doesNotMatch(build, /backend\/package\.json/);
  assert.equal(version("apps/k7/package.json"), version("package.json"));
});
