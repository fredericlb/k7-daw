// macNotarize.mjs — signs a DMG, has Apple notarize it and staples the ticket: `node macNotarize.mjs <file.dmg>`.
// Tauri notarizes the .app itself; the DMG that carries it is ours. Needs a Developer ID and an App Store Connect
// API key, or it refuses (docs/macos.md).

import { execFileSync } from "node:child_process";

const NEED = [
  "APPLE_SIGNING_IDENTITY",
  "APPLE_API_ISSUER",
  "APPLE_API_KEY",
  "APPLE_API_KEY_PATH",
];

/** The `xcrun notarytool submit` arguments for `dmg`, from the environment `env`. */
export function submitArgs(dmg, env) {
  return [
    "notarytool",
    "submit",
    dmg,
    "--key",
    env.APPLE_API_KEY_PATH,
    "--key-id",
    env.APPLE_API_KEY,
    "--issuer",
    env.APPLE_API_ISSUER,
    "--wait",
  ];
}

export function notarize(dmg, env = process.env) {
  const missing = NEED.filter((k) => !env[k]);
  if (missing.length) throw new Error(`missing: ${missing.join(", ")}`);
  if (env.APPLE_SIGNING_IDENTITY === "-")
    throw new Error("an ad-hoc signature cannot be notarized");
  const run = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit" });
  run("codesign", [
    "--force",
    "--sign",
    env.APPLE_SIGNING_IDENTITY,
    "--timestamp",
    dmg,
  ]);
  run("xcrun", submitArgs(dmg, env));
  run("xcrun", ["stapler", "staple", dmg]);
}

if (process.argv[1]?.endsWith("macNotarize.mjs")) {
  const dmg = process.argv[2];
  if (!dmg) throw new Error("usage: node macNotarize.mjs <file.dmg>");
  notarize(dmg);
}
