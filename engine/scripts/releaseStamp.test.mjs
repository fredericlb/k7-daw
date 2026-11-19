// releaseStamp.test.mjs — the stamp rules on the three builds there are: a tag, a commit of main, and an unstamped
// one. The expected values are what the Rust, Gradle and Kotlin copies answer, so a drift here is a drift there.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { isStamp, versionFor } from "./releaseStamp.mjs";
import { packageTray } from "./trayRelease.mjs";

const CLI = fileURLToPath(new URL("./releaseStamp.mjs", import.meta.url));
const ROOT = JSON.parse(
  readFileSync(new URL("../../package.json", import.meta.url), "utf8"),
).version;
const cli = (...args) =>
  execFileSync(process.execPath, [CLI, ...args], { encoding: "utf8" }).trim();

const CASES = [
  { stamp: "v1.2.3", stamped: true, version: "0.1.0" },
  {
    stamp: "snapshot-9f2c1ab",
    stamped: true,
    version: "0.1.0-snapshot-9f2c1ab",
  },
  { stamp: "dev", stamped: false, version: "0.1.0" },
  { stamp: "", stamped: false, version: "0.1.0" },
];

describe("releaseStamp", () => {
  for (const c of CASES) {
    it(`reads '${c.stamp}'`, () => {
      assert.equal(isStamp(c.stamp), c.stamped);
      assert.equal(versionFor("0.1.0", c.stamp), c.version);
    });
  }

  it("refuses a shape no build is stamped with", () => {
    for (const s of ["v1.2", "v1.2.3-rc1", "snapshot-9F2C1AB", "snapshot-abc"])
      assert.equal(isStamp(s), false, s);
  });

  it("answers a shell from the command line", () => {
    assert.equal(cli("version", "v1.2.3"), ROOT);
    assert.equal(
      cli("version", "snapshot-9f2c1ab"),
      `${ROOT}-snapshot-9f2c1ab`,
    );
    assert.equal(cli("version", ""), ROOT);
  });

  it("names every installer for its stamp, and the dmg is the macOS one", () => {
    const root = mkdtempSync(join(tmpdir(), "release-stamp-"));
    try {
      const bundle = join(root, "bundle");
      mkdirSync(join(bundle, "dmg"), { recursive: true });
      writeFileSync(join(bundle, "dmg", "K7 DAW_0.1.0_aarch64.dmg"), "");
      const release = mkdtempSync(join(root, "out-"));
      packageTray({
        bundle,
        release,
        stamp: "v1.2.3",
        platform: "macos",
        arch: "arm64",
      });
      assert.ok(existsSync(join(release, "k7-daw_v1.2.3_macos-arm64.dmg")));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses a bundle with no installer", () => {
    const root = mkdtempSync(join(tmpdir(), "release-stamp-"));
    try {
      assert.throws(
        () =>
          packageTray({
            bundle: join(root, "none"),
            release: root,
            stamp: "dev",
            platform: "linux",
            arch: "x64",
          }),
        /no installer/,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
