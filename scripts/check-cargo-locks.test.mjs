// Tests of the lock deny-list: an enabled entry refuses a lock that pins it; a disabled one is listed, not enforced.
// Run: node --test scripts/check-cargo-locks.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { DENY, denied, pinned } from "./cargoLockDeny.mjs";

const fixture = readFileSync(
  new URL("./fixtures/locks/with-rquickjs.lock", import.meta.url),
  "utf8",
);

test("reads the package names of a lock", () => {
  assert.deepEqual(pinned(fixture), ["k7-js", "rquickjs", "rquickjs-sys"]);
});

test("an enabled entry refuses a lock that pins it", () => {
  const on = DENY.map((d) => ({ ...d, enabled: true }));
  assert.deepEqual(
    denied(fixture, on).map((d) => d.name),
    ["rquickjs", "rquickjs-sys"],
  );
});

test("a lock without the crate passes", () => {
  const on = DENY.map((d) => ({ ...d, enabled: true }));
  assert.deepEqual(denied('[[package]]\nname = "k7-js"\n', on), []);
});

test("the list names rquickjs and rquickjs-sys, both enforced", () => {
  assert.deepEqual(
    DENY.map((d) => d.name),
    ["rquickjs", "rquickjs-sys"],
  );
  assert.ok(DENY.every((d) => d.enabled));
});
