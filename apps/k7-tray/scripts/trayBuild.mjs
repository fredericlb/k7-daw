// trayBuild.mjs — `tauri build` for K7 DAW, with the updater's signing wired in only when both halves of the key are
// there: the private key signs the update archives, the public one is the one the app checks them against.
// A build with neither is a local build and writes no updater archives (docs/tray.md).

import { spawnSync } from "node:child_process";

/** The `--config` argument a signed build adds, none for a local one; half a key is an error, never a silent unsigned build. */
export function signingConfig(env) {
  const pub = env.K7_UPDATER_PUBKEY;
  const priv = env.TAURI_SIGNING_PRIVATE_KEY;
  if (!pub && !priv) return [];
  if (!pub || !priv)
    throw new Error(
      "an updater key needs both K7_UPDATER_PUBKEY and TAURI_SIGNING_PRIVATE_KEY",
    );
  return [
    "--config",
    JSON.stringify({
      bundle: { createUpdaterArtifacts: true },
      plugins: { updater: { pubkey: pub } },
    }),
  ];
}

if (process.argv[1]?.endsWith("trayBuild.mjs")) {
  const args = [
    "tauri",
    "build",
    ...signingConfig(process.env),
    ...process.argv.slice(2),
  ];
  const r = spawnSync("npx", args, { stdio: "inherit" });
  process.exit(r.status ?? 1);
}
