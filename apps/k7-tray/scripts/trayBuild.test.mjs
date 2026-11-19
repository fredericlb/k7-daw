import assert from "node:assert/strict";
import test from "node:test";
import { signingConfig } from "./trayBuild.mjs";

test("a local build adds nothing, a signed one turns the archives on with the public key", () => {
  assert.deepEqual(signingConfig({}), []);
  const [flag, json] = signingConfig({
    K7_UPDATER_PUBKEY: "PUB",
    TAURI_SIGNING_PRIVATE_KEY: "PRIV",
  });
  assert.equal(flag, "--config");
  const c = JSON.parse(json);
  assert.equal(c.bundle.createUpdaterArtifacts, true);
  assert.equal(c.plugins.updater.pubkey, "PUB");
});

test("half a key is refused", () => {
  assert.throws(() => signingConfig({ K7_UPDATER_PUBKEY: "PUB" }));
  assert.throws(() => signingConfig({ TAURI_SIGNING_PRIVATE_KEY: "PRIV" }));
});
