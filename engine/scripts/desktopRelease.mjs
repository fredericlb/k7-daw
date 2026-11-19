// trayRelease.mjs — K7 DAW's installers from what `tauri build` left in apps/k7-tray/src-tauri/target, copied into
// the release folder under a name made of the stamp, like every other file of a release (docs/release.md).

import { copyFileSync, existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

/** Tauri's bundle folders, and what each installer becomes. */
const KINDS = [
  { dir: "nsis", match: /-setup\.exe$/, ext: ".exe" },
  { dir: "appimage", match: /\.AppImage$/, ext: ".AppImage" },
  { dir: "deb", match: /\.deb$/, ext: ".deb" },
  { dir: "dmg", match: /\.dmg$/, ext: ".dmg" },
];

/** Copy the installers into `release`, renamed for the stamp. */
export function packageTray({ bundle, release, stamp, platform, arch }) {
  const name = (ext) => `k7-daw_${stamp}_${platform}-${arch}${ext}`;
  let found = 0;
  for (const k of KINDS) {
    const dir = join(bundle, k.dir);
    if (!existsSync(dir)) continue;
    for (const f of readdirSync(dir).filter((f) => k.match.test(f))) {
      copyFileSync(join(dir, f), join(release, name(k.ext)));
      console.log(`✔ ${join(release, name(k.ext))}`);
      found++;
    }
  }
  if (!found)
    throw new Error(
      `no installer under ${bundle} — run \`npm run tray:build --workspace k7-tray\` first`,
    );
}
