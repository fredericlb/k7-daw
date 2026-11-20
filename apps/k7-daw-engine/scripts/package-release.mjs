#!/usr/bin/env node
// package-release.mjs — one release file per kind, for the platform that produced it, into dist/release/:
// `engine` archives dist/k7-runtime/ (build-engine.mjs); `desktop` renames the installers apps/k7-desktop built (desktopRelease.mjs).
// Named for the root package.json's version. See docs/release.md.

import { existsSync, mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { distDir, run, windowsTar } from "./buildTools.mjs";
import { packageDesktop } from "./desktopRelease.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const RELEASE = join(distDir(repo), "release");

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

const { values: args } = parseArgs({ options: { kind: { type: "string" } } });
const kind = args.kind;
if (kind !== "engine" && kind !== "desktop")
  fail("usage: package-release.mjs --kind engine|desktop");
const version = JSON.parse(
  readFileSync(join(repo, "package.json"), "utf8"),
).version;
const platform = { win32: "windows", darwin: "macos", linux: "linux" }[
  process.platform
];
if (!platform) fail(`no release for ${process.platform}.`);
const arch = process.arch === "arm64" ? "arm64" : "x64";
mkdirSync(RELEASE, { recursive: true });

if (kind === "engine") {
  const staged = join(distDir(repo), "k7-runtime");
  if (!existsSync(staged))
    fail(`${staged} is missing — run engine/scripts/build-engine.mjs first.`);
  const name = `k7-runtime_${version}_${platform}-${arch}`;
  // tar.gz outside Windows: the binaries are only useful with their executable bit.
  const archive = `${name}.${platform === "windows" ? "zip" : "tar.gz"}`;
  rmSync(join(RELEASE, archive), { force: true });
  const parent = distDir(repo);
  if (platform === "windows")
    // bsdtar writes .zip from the extension.
    run(
      windowsTar(),
      ["-a", "-c", "-f", join(RELEASE, archive), "k7-runtime"],
      parent,
    );
  else run("tar", ["-czf", join(RELEASE, archive), "k7-runtime"], parent);
  console.log(
    `\n✔ ${join(RELEASE, archive)}  (${(statSync(join(RELEASE, archive)).size / 1e6).toFixed(1)} MB)`,
  );
} else {
  packageDesktop({
    bundle: join(
      repo,
      "apps",
      "k7-desktop",
      "src-tauri",
      "target",
      "release",
      "bundle",
    ),
    release: RELEASE,
    version,
    platform,
    arch,
  });
}
