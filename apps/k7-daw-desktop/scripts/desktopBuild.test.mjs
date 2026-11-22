// desktopBuild.test.mjs — the iCloud entitlements a profile's team gives, and the config that embeds the profile.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { entitlementsWith, profileConfig } from "./desktopBuild.mjs";

const base = readFileSync(
  join(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    "src-tauri",
    "macos",
    "app.entitlements",
  ),
  "utf8",
);

test("the iCloud keys join the app's own, named for the profile's prefix and team", () => {
  const out = entitlementsWith(base, {
    prefix: "S5TM69LZLJ",
    team: "S5TM69LZLJ",
  });
  assert.ok(out.includes("<key>com.apple.security.cs.allow-jit</key>"));
  assert.ok(out.includes("<string>S5TM69LZLJ.rocks.k7.daw</string>"));
  assert.ok(out.includes("<key>keychain-access-groups</key>"));
  assert.ok(
    out.includes("<key>com.apple.developer.ubiquity-kvstore-identifier</key>"),
  );
  assert.ok(!out.includes("$("), "codesign expands no variable");
  assert.ok(out.trimEnd().endsWith("</plist>"));
});

test("the config swaps the entitlements and puts the profile in Contents", () => {
  const c = profileConfig("/t/app.entitlements", "/p/k7.provisionprofile");
  assert.equal(c.bundle.macOS.entitlements, "/t/app.entitlements");
  assert.equal(
    c.bundle.macOS.files["embedded.provisionprofile"],
    "/p/k7.provisionprofile",
  );
});
