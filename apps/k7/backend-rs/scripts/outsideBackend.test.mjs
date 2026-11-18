// outsideBackend.test.mjs — what outlives apps/k7/backend reads nothing from it: the front's version and k7-core's seed.
// Run by the tray's workspace, which stages from this folder (apps/k7-tray/package.json).

import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
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
  const front = read("apps/k7/docker/Dockerfile")
    .split("FROM ")
    .find((s) => s.includes("AS front-build"));
  assert.match(front, /COPY apps\/k7\/package\.json /);
  assert.doesNotMatch(front, /apps\/k7\/backend\//);
});

test("k7-core's seed is archived from here, and nobody imports it from the backend", () => {
  const seen = [];
  const walk = (rel) => {
    for (const e of readdirSync(join(repo, rel), { withFileTypes: true })) {
      if (
        ["node_modules", "target", "dist", "resources", ".git"].includes(
          e.name,
        ) ||
        e.name.startsWith("target-")
      )
        continue;
      const r = `${rel}/${e.name}`;
      if (e.isDirectory()) walk(r);
      else if (
        /\.(m?[jt]s|kts)$/.test(e.name) &&
        /backend\/scripts\/libraryArchive/.test(read(r))
      )
        seen.push(r);
    }
  };
  for (const root of ["apps", "engine", "packages", "scripts"]) walk(root);
  assert.deepEqual(
    seen.filter((f) => !f.endsWith("outsideBackend.test.mjs")),
    [],
  );
  assert.match(
    read("apps/k7/backend-rs/scripts/libraryArchive.mjs"),
    /export async function bundleLibrary/,
  );
});
