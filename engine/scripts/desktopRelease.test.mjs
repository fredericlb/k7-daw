// desktopRelease.test.mjs — K7 DAW's installers are renamed for the version, and a bundle with none is refused.

import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { packageDesktop } from "./desktopRelease.mjs";

describe("desktopRelease", () => {
  it("names every installer for the version, and the dmg is the macOS one", () => {
    const root = mkdtempSync(join(tmpdir(), "desktop-release-"));
    try {
      const bundle = join(root, "bundle");
      mkdirSync(join(bundle, "dmg"), { recursive: true });
      writeFileSync(join(bundle, "dmg", "K7 DAW_0.1.0_aarch64.dmg"), "");
      const release = mkdtempSync(join(root, "out-"));
      packageDesktop({
        bundle,
        release,
        version: "1.2.3",
        platform: "macos",
        arch: "arm64",
      });
      assert.ok(existsSync(join(release, "k7-daw_1.2.3_macos-arm64.dmg")));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses a bundle with no installer", () => {
    const root = mkdtempSync(join(tmpdir(), "desktop-release-"));
    try {
      assert.throws(
        () =>
          packageDesktop({
            bundle: join(root, "none"),
            release: root,
            version: "1.2.3",
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
