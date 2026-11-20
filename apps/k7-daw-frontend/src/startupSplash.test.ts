// startupSplash.test.ts — the cassette is ONE width on every layer that can show it: Android's launch
// screen, iOS's, the phone's shell page and the web splash. A second number is a jump from one layer to
// the next that nobody notices until a screen wide enough. See docs/icons.md.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const read = (p: string) => readFileSync(join(repo, p));
const text = (p: string) => read(p).toString("utf8");

/** The one width, out of the script that renders every splash asset. */
function theWidth(): number {
  const named = /const SPLASH_WIDTH = (\d+);/.exec(
    text("apps/k7/frontend/scripts/icons.mjs"),
  );
  if (!named) throw new Error("icons.mjs names no SPLASH_WIDTH");
  return Number(named[1]);
}

/** A PNG's width in pixels, read from its IHDR. */
const pngWidth = (p: string) => read(p).readUInt32BE(16);

describe("the startup cassette's width", () => {
  const width = theWidth();

  it("is the web splash's, as index.html sets it", () => {
    expect(text("apps/k7/frontend/index.html")).toContain(
      `.k7-splash img { width: ${width}px; height: auto; }`,
    );
  });

  it("is the shell page's, where the phone draws it", () => {
    expect(text("engine/k7-android/web/src/Backend.tsx")).toContain(
      `w-[${width}px]`,
    );
  });

  it("is Android's launch screen, whose height is the drawing's own ratio", () => {
    const xml = text("engine/k7-android/app/src/main/res/drawable/splash.xml");
    expect(xml).toContain(`android:width="${width}dp"`);
    const box = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(
      text("apps/k7/frontend/icon/k7-splash.svg"),
    );
    if (!box) throw new Error("the splash drawing names no viewBox");
    const height = Math.round((width * Number(box[2])) / Number(box[1]));
    expect(xml).toContain(`android:height="${height}dp"`);
  });

  it("is iOS's launch screen, in points at both scales", () => {
    const dir = "engine/k7-ios/k7/Assets.xcassets/Splash.imageset";
    expect(pngWidth(`${dir}/splash@2x.png`) / 2).toBe(width);
    expect(pngWidth(`${dir}/splash@3x.png`) / 3).toBe(width);
  });
});
