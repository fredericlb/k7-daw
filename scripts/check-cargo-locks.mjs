#!/usr/bin/env node
// check-cargo-locks.mjs — `npm run lint:locks`: each Rust workspace's Cargo.lock still pins every dependency, and pins
// no crate of the deny-list (`cargoLockDeny.mjs`). The tray links the backend's crates by path (docs/lints.md).
// Without cargo on the PATH (WSL) the staleness half says so and passes; the deny-list needs no cargo.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { DENY, denied } from "./cargoLockDeny.mjs";

const LOCKED = [
  "apps/k7/backend-rs",
  "apps/k7-tray/src-tauri",
  "packages/song-schema/rust",
  "packages/k7-wasm/rust",
  "packages/song/rust",
];

const refused = LOCKED.flatMap((dir) => {
  const lock = join(dir, "Cargo.lock");
  if (!existsSync(lock)) return [];
  return denied(readFileSync(lock, "utf8")).map(
    (d) => `lint:locks: ${lock} pins ${d.name}: ${d.why}`,
  );
});
if (refused.length) {
  console.error(refused.join("\n"));
  process.exit(1);
}
console.log(
  `lint:locks: no lock pins any of the ${DENY.filter((d) => d.enabled).length} denied crates`,
);

const probe = spawnSync("cargo", ["--version"], { encoding: "utf8" });
if (probe.error || probe.status !== 0) {
  console.log(
    "lint:locks: no cargo on this machine, skipped — OneDev's lint and test checks the locks",
  );
  process.exit(0);
}

const stale = LOCKED.filter((dir) => {
  // cargo metadata writes target/.rustc_info.json; keep it out of the tree (gate.sh refuses one).
  const r = spawnSync(
    "cargo",
    ["metadata", "--locked", "--format-version", "1"],
    {
      cwd: dir,
      encoding: "utf8",
      maxBuffer: 256 << 20,
      env: {
        ...process.env,
        CARGO_TARGET_DIR: join(homedir(), ".cache", "k7-cargo-metadata"),
      },
    },
  );
  if (r.status === 0) return false;
  console.error(`lint:locks: ${dir}/Cargo.lock\n${r.stderr.trim()}`);
  return true;
});
if (stale.length) {
  console.error(
    "lint:locks: run `cargo metadata --format-version 1 >/dev/null` in each folder above and commit its Cargo.lock",
  );
  process.exit(1);
}
console.log(
  `lint:locks: ${LOCKED.length} Cargo.lock files pin their manifests`,
);
