import assert from "node:assert/strict";
import test from "node:test";
import { codesignArgs, order } from "./macSign.mjs";

test("libraries are signed before the programs that carry them", () => {
  const got = order([
    "/r/bin/k7-runtime",
    "/r/lib/libonnx.dylib",
    "/r/node/node",
    "/r/x/a.node",
  ]);
  assert.deepEqual(got.slice(0, 2).sort(), [
    "/r/lib/libonnx.dylib",
    "/r/x/a.node",
  ]);
});

test("an ad-hoc identity takes no timestamp, a real one does", () => {
  assert.ok(
    codesignArgs("/r/bin/k7-supervisor", "-").includes("--timestamp=none"),
  );
  assert.ok(
    codesignArgs(
      "/r/bin/k7-supervisor",
      "Developer ID Application: X",
    ).includes("--timestamp"),
  );
});

test("node and the engine carry their entitlements, the supervisor the hardened runtime alone", () => {
  assert.ok(
    codesignArgs("/r/node/node", "-").some((a) =>
      a.endsWith("node.entitlements"),
    ),
  );
  assert.ok(
    codesignArgs("/r/bin/k7-runtime", "-").some((a) =>
      a.endsWith("engine.entitlements"),
    ),
  );
  const sup = codesignArgs("/r/bin/k7-supervisor", "-");
  assert.ok(!sup.includes("--entitlements"));
  assert.ok(sup.includes("runtime"));
});

test("notarization refuses an ad-hoc identity and a missing key before it runs anything", async () => {
  const { notarize, submitArgs } = await import("./macNotarize.mjs");
  assert.throws(
    () => notarize("x.dmg", { APPLE_SIGNING_IDENTITY: "-" }),
    /missing/,
  );
  const env = {
    APPLE_SIGNING_IDENTITY: "-",
    APPLE_API_ISSUER: "i",
    APPLE_API_KEY: "k",
    APPLE_API_KEY_PATH: "/p.p8",
  };
  assert.throws(() => notarize("x.dmg", env), /ad-hoc/);
  assert.deepEqual(submitArgs("x.dmg", env).slice(0, 3), [
    "notarytool",
    "submit",
    "x.dmg",
  ]);
});
