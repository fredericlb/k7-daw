import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  cleanCameraName,
  matchRemoteCamera,
  type RemoteCamera,
  readCameraFrame,
  uniqueCameraName,
} from "./cameras.ts";

// What the Rust front's camera hub must answer as the browser's helpers do (apps/k7-daw-backend/docs/routes.md).
const file = join(
  import.meta.dirname,
  "../../../k7-daw-backend/fixtures/camera-vectors.json",
);
const v: {
  clean: { raw: string; fallback?: string; out: string }[];
  unique: { want: string; taken: string[]; out: string }[];
  match: { label: string; cameras: RemoteCamera[]; out: unknown }[];
  frame: { text: string; out: unknown }[];
} = JSON.parse(readFileSync(file, "utf8"));

describe("camera vectors", () => {
  it("are what the TypeScript helpers answer today", () => {
    for (const c of v.clean)
      expect(
        c.fallback === undefined
          ? cleanCameraName(c.raw)
          : cleanCameraName(c.raw, c.fallback),
      ).toBe(c.out);
    for (const u of v.unique)
      expect(uniqueCameraName(u.want, u.taken)).toBe(u.out);
    for (const m of v.match)
      expect(matchRemoteCamera(m.label, m.cameras)).toEqual(m.out);
    for (const f of v.frame) expect(readCameraFrame(f.text)).toEqual(f.out);
  });
});
