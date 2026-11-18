// releaseStamp.test.mjs — the stamp rules on the three builds there are: a tag, a commit of main, and an unstamped
// one. The expected values are what the Rust, Gradle and Kotlin copies answer, so a drift here is a drift there.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
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
import { channelOf, isStamp, versionFor } from "./releaseStamp.mjs";
import { packageTray } from "./trayRelease.mjs";

const CLI = fileURLToPath(new URL("./releaseStamp.mjs", import.meta.url));
const ROOT = JSON.parse(
  readFileSync(new URL("../../package.json", import.meta.url), "utf8"),
).version;
const cli = (...args) =>
  execFileSync(process.execPath, [CLI, ...args], { encoding: "utf8" }).trim();

const CASES = [
  { stamp: "v1.2.3", stamped: true, channel: "release", version: "0.1.0" },
  {
    stamp: "snapshot-9f2c1ab",
    stamped: true,
    channel: "snapshot",
    version: "0.1.0-snapshot-9f2c1ab",
  },
  { stamp: "dev", stamped: false, channel: null, version: "0.1.0" },
  { stamp: "", stamped: false, channel: null, version: "0.1.0" },
];

describe("releaseStamp", () => {
  for (const c of CASES) {
    it(`reads '${c.stamp}'`, () => {
      assert.equal(isStamp(c.stamp), c.stamped);
      assert.equal(channelOf(c.stamp), c.channel);
      assert.equal(versionFor("0.1.0", c.stamp), c.version);
    });
  }

  it("refuses a shape CI does not make", () => {
    for (const s of ["v1.2", "v1.2.3-rc1", "snapshot-9F2C1AB", "snapshot-abc"])
      assert.equal(channelOf(s), null, s);
  });

  it("answers a shell from the command line", () => {
    assert.equal(cli("channel", "v1.2.3"), "release");
    assert.equal(cli("channel", "snapshot-9f2c1ab"), "snapshot");
    assert.throws(() => cli("channel", "dev"));
    assert.equal(cli("version", "v1.2.3"), ROOT);
    assert.equal(
      cli("version", "snapshot-9f2c1ab"),
      `${ROOT}-snapshot-9f2c1ab`,
    );
    assert.equal(cli("version", ""), ROOT);
  });

  it("names the tray's updater URLs under the channel, and `snapshot` for an unstamped build", () => {
    const root = mkdtempSync(join(tmpdir(), "release-stamp-"));
    try {
      const bundle = join(root, "bundle");
      mkdirSync(join(bundle, "nsis"), { recursive: true });
      writeFileSync(join(bundle, "nsis", "k7-tray-setup.exe"), "");
      writeFileSync(join(bundle, "nsis", "k7-tray-setup.exe.sig"), "sig\n");
      const url = (stamp) => {
        const release = mkdtempSync(join(root, "out-"));
        const f = packageTray({
          bundle,
          release,
          stamp,
          platform: "windows",
          arch: "x64",
        });
        return f["windows-x86_64"].url;
      };
      const at = "https://k7.rocks/app_downloads/tray";
      assert.equal(
        url("v1.2.3"),
        `${at}/release/k7-tray_v1.2.3_windows-x64.exe`,
      );
      assert.equal(
        url("snapshot-9f2c1ab"),
        `${at}/snapshot/k7-tray_snapshot-9f2c1ab_windows-x64.exe`,
      );
      assert.equal(url("dev"), `${at}/snapshot/k7-tray_dev_windows-x64.exe`);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
