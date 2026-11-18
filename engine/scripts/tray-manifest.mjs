#!/usr/bin/env node
// tray-manifest.mjs — `--dir <dist> --version <semver>`: merge the per-platform fragments trayRelease.mjs wrote into
// the one latest.json tauri-plugin-updater reads, then drop the fragments. Run when a tray release is published.
// See docs/app-downloads.md.

import { readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

const {
  values: { dir, version },
} = parseArgs({
  options: { dir: { type: "string" }, version: { type: "string" } },
});
if (!dir || !version) {
  console.error("usage: tray-manifest.mjs --dir <dist> --version <semver>");
  process.exit(1);
}

const fragments = readdirSync(dir).filter((f) =>
  /^k7-tray-updater_.*\.json$/.test(f),
);
const platforms = {};
for (const f of fragments) {
  Object.assign(platforms, JSON.parse(readFileSync(join(dir, f), "utf8")));
  rmSync(join(dir, f));
}
if (!Object.keys(platforms).length) {
  console.log(
    "no signed tray build: no latest.json, the installed trays keep their build",
  );
  process.exit(0);
}
const manifest = { version, pub_date: new Date().toISOString(), platforms };
writeFileSync(
  join(dir, "latest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
console.log(`latest.json: ${version} for ${Object.keys(platforms).join(", ")}`);
