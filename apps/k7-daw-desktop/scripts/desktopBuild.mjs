// desktopBuild.mjs — `tauri build`, plus on a Mac the iCloud entitlements and the embedded provisioning profile when
// `K7_MAC_PROVISIONING_PROFILE` names one (docs/icloud.md). What a reader would get wrong: without a profile the
// build is exactly the plain one, since macOS kills an app that carries these entitlements with no profile allowing them.

import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const BASE = join(here, "..", "src-tauri", "macos", "app.entitlements");
export const BUNDLE_ID = "rocks.k7.daw";

const string = (k, v) => `  <key>${k}</key>\n  <string>${v}</string>\n`;

/** The app's own entitlements plus iCloud's, for the App ID prefix and team a profile names. */
export function entitlementsWith(base, { prefix, team }) {
  const app = `${prefix}.${BUNDLE_ID}`;
  const extra =
    string("com.apple.application-identifier", app) +
    string("com.apple.developer.team-identifier", team) +
    `  <key>keychain-access-groups</key>\n  <array>\n    <string>${app}</string>\n  </array>\n` +
    string(
      "com.apple.developer.ubiquity-kvstore-identifier",
      `${team}.${BUNDLE_ID}`,
    );
  const at = base.lastIndexOf("</dict>");
  if (at < 0) throw new Error("the app's entitlements have no <dict>");
  return base.slice(0, at) + extra + base.slice(at);
}

/** The `tauri build --config` that swaps the entitlements and embeds the profile. */
export function profileConfig(entitlements, profile) {
  return {
    bundle: {
      macOS: {
        entitlements,
        files: { "embedded.provisionprofile": profile },
      },
    },
  };
}

/** The App ID prefix and team of a profile, refused when it does not allow k7's iCloud. */
export function readProfile(profile) {
  const plist = join(mkdtempSync(join(tmpdir(), "k7-profile-")), "p.plist");
  writeFileSync(
    plist,
    execFileSync("security", ["cms", "-D", "-i", profile], {
      stdio: ["ignore", "pipe", "inherit"],
    }),
  );
  const get = (path) => {
    try {
      return execFileSync(
        "/usr/libexec/PlistBuddy",
        ["-c", `Print :${path}`, plist],
        {
          stdio: ["ignore", "pipe", "ignore"],
        },
      )
        .toString()
        .trim();
    } catch {
      return "";
    }
  };
  const app = get("Entitlements:com.apple.application-identifier");
  const team = get("Entitlements:com.apple.developer.team-identifier");
  if (!app.endsWith(`.${BUNDLE_ID}`) || !team)
    throw new Error(`${profile} is not a profile of the App ID ${BUNDLE_ID}`);
  if (!get("Entitlements:com.apple.developer.ubiquity-kvstore-identifier"))
    throw new Error(
      `${profile} does not allow iCloud key-value storage: enable iCloud on the App ID, then download the profile again`,
    );
  return { prefix: app.slice(0, -BUNDLE_ID.length - 1), team };
}

function main(args) {
  const profile = process.env.K7_MAC_PROVISIONING_PROFILE;
  const extra = [];
  if (process.platform === "darwin" && profile) {
    const identity = process.env.APPLE_SIGNING_IDENTITY;
    if (!identity || identity === "-")
      throw new Error(
        "K7_MAC_PROVISIONING_PROFILE needs APPLE_SIGNING_IDENTITY: the certificate the profile lists",
      );
    const at = resolve(profile);
    const ids = readProfile(at);
    const ent = join(
      mkdtempSync(join(tmpdir(), "k7-icloud-")),
      "app.entitlements",
    );
    writeFileSync(ent, entitlementsWith(readFileSync(BASE, "utf8"), ids));
    extra.push("--config", JSON.stringify(profileConfig(ent, at)));
    console.log(`· iCloud sync is on for this build (team ${ids.team})`);
  } else if (process.platform === "darwin") {
    console.log(
      "· iCloud sync is off for this build: no K7_MAC_PROVISIONING_PROFILE (docs/icloud.md)",
    );
  }
  // The config goes before the caller's arguments, which may end with `--` and cargo's own.
  const r = spawnSync("tauri", ["build", ...extra, ...args], {
    stdio: "inherit",
  });
  process.exit(r.status ?? 1);
}

if (process.argv[1] === fileURLToPath(import.meta.url))
  main(process.argv.slice(2));
