#!/usr/bin/env node
// check-version.mjs — the release version has ONE source, the root package.json's `version`; this keeps
// every copy equal: each private workspace's package.json, its package-lock entry, the iOS project's
// MARKETING_VERSION.
// `--tag <vX.Y.Z>` also refuses a release tag that is not `v` + that version; `--set <X.Y.Z>` rewrites every
// copy, the root's included. See docs/versioning.md.

import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const PBXPROJ = "engine/k7-ios/k7.xcodeproj/project.pbxproj";
const SEMVER = /^\d+\.\d+\.\d+$/;

const { values: args } = parseArgs({
  options: { tag: { type: "string" }, set: { type: "string" } },
});
const read = (rel) => readFileSync(join(repo, rel), "utf8");

// The workspace folders the root names.
function workspaceDirs() {
  const dirs = [];
  for (const pattern of JSON.parse(read("package.json")).workspaces) {
    if (!pattern.endsWith("/*")) dirs.push(pattern);
    else {
      const parent = pattern.slice(0, -2);
      for (const e of readdirSync(join(repo, parent), { withFileTypes: true }))
        if (e.isDirectory()) dirs.push(`${parent}/${e.name}`);
    }
  }
  return dirs.filter((d) => existsSync(join(repo, d, "package.json")));
}

// Replaces the manifest's own `"version"`, the first one, so every other byte of the file stays.
const withVersion = (text, version) =>
  text.replace(/("version":\s*")[^"]*(")/, `$1${version}$2`);

const set = args.set;
if (set !== undefined) {
  if (!SEMVER.test(set)) {
    console.error(`--set wants X.Y.Z, not '${set}'`);
    process.exit(1);
  }
  writeFileSync(
    join(repo, "package.json"),
    withVersion(read("package.json"), set),
  );
}

const version = JSON.parse(read("package.json")).version;
const errors = [];
if (!SEMVER.test(version))
  errors.push(`package.json: version '${version}' is not X.Y.Z`);

const lock = JSON.parse(read("package-lock.json"));
const lockPaths = [""];
for (const dir of workspaceDirs()) {
  const file = `${dir}/package.json`;
  const manifest = JSON.parse(read(file));
  if (manifest.private !== true) {
    errors.push(`${file}: not private`);
    continue;
  }
  lockPaths.push(dir);
  if (set !== undefined)
    writeFileSync(join(repo, file), withVersion(read(file), set));
  else if (manifest.version !== version)
    errors.push(`${file}: version ${manifest.version}, expected ${version}`);
}

if (set !== undefined) {
  lock.version = set;
  for (const p of lockPaths)
    if (lock.packages[p]) lock.packages[p].version = set;
  writeFileSync(
    join(repo, "package-lock.json"),
    `${JSON.stringify(lock, null, 2)}\n`,
  );
  writeFileSync(
    join(repo, PBXPROJ),
    read(PBXPROJ).replace(
      /MARKETING_VERSION = [^;]*;/g,
      `MARKETING_VERSION = ${set};`,
    ),
  );
} else {
  if (lock.version !== version)
    errors.push(
      `package-lock.json: version ${lock.version}, expected ${version}`,
    );
  for (const p of lockPaths) {
    const v = lock.packages[p]?.version;
    if (v !== version)
      errors.push(`package-lock.json: "${p}" at ${v}, expected ${version}`);
  }
  const marketing = [
    ...read(PBXPROJ).matchAll(/MARKETING_VERSION = ([^;]*);/g),
  ].map((m) => m[1]);
  if (!marketing.length) errors.push(`${PBXPROJ}: no MARKETING_VERSION`);
  for (const m of marketing)
    if (m !== version)
      errors.push(`${PBXPROJ}: MARKETING_VERSION ${m}, expected ${version}`);
}

const tag = args.tag;
if (tag !== undefined && tag !== `v${version}`)
  errors.push(
    `tag '${tag}' is not v${version}: bump the version (--set) or tag the version it holds`,
  );

if (errors.length) {
  for (const e of errors) console.error(`✘ ${e}`);
  console.error(
    "\nOne version, the root package.json's; `node scripts/check-version.mjs --set X.Y.Z` writes every copy.",
  );
  process.exit(1);
}
console.log(
  set !== undefined
    ? `version set to ${set} everywhere`
    : `version ${version} everywhere`,
);
