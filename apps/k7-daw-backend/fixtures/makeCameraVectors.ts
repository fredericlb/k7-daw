// makeCameraVectors.ts — writes camera-vectors.json from the TypeScript helpers the browser runs; `ice` is kept as committed.
// Run with `npx tsx apps/k7-daw-backend/fixtures/makeCameraVectors.ts`; both sides' tests then check it (apps/k7-daw-backend/docs/routes.md).
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  cleanCameraName,
  matchRemoteCamera,
  readCameraFrame,
  uniqueCameraName,
} from "@studio/web-protocol/cameras";

const clean: { raw: string; fallback?: string }[] = [
  { raw: "  a\nb\t c " },
  { raw: "   " },
  { raw: "" },
  { raw: "", fallback: "phone" },
  { raw: "x".repeat(100) },
  { raw: "Pixel 8 · Chrome" },
  { raw: "tab\u0001ctl\u007fdel" },
  { raw: " nbsp　ideo﻿bom ls" },
  { raw: "nel\u0085kept" },
  { raw: "Frédéric 🎛 Ünïcode" },
  { raw: `${"y".repeat(58)}🎥z` },
  { raw: `${"w".repeat(59)} tail` },
];

const unique: { want: string; taken: string[] }[] = [
  { want: "iPad", taken: [] },
  { want: "iPad", taken: ["ipad"] },
  { want: "iPad", taken: ["iPad", "iPad (2)"] },
  { want: "iPad", taken: ["iPad", "ipad (3)"] },
  { want: "Écran", taken: ["écran"] },
];

const cams = [
  { name: "Pixel 8 · Chrome", since: 10 },
  { name: "iPad", since: 30 },
  { name: "Stage left", since: 20 },
];
const tied = [
  { name: "first", since: 5 },
  { name: "second", since: 5 },
];
const match: { label: string; cameras: { name: string; since: number }[] }[] = [
  { label: "remote:iPad", cameras: [] },
  { label: "remote:", cameras: cams },
  { label: "remote:   ", cameras: cams },
  { label: "remote:Stage left", cameras: cams },
  { label: "remote:stage LEFT", cameras: cams },
  { label: "remote:pixel", cameras: cams },
  { label: "remote:my iPad pro", cameras: cams },
  { label: "remote:Drums", cameras: cams },
  { label: "remote:x", cameras: tied },
  { label: "", cameras: cams },
  { label: "Stage left", cameras: cams },
  { label: "remote:ÉCRAN", cameras: [{ name: "écran", since: 1 }] },
];

const frame: string[] = [
  '{"type":"gone","from":"v1"}',
  '{"type":"offer","to":"v","sdp":{"type":"offer","sdp":"o"},"extra":[1,2.5,null]}',
  '  {"type":"want","label":"remote:"}  ',
  '{"type":3}',
  "[]",
  "null",
  "nope",
  "42",
  '"type"',
  '{"label":"x"}',
  '{"type":"a","type":"b"}',
];

const file = join(import.meta.dirname, "camera-vectors.json");
// The hub's ICE servers have no TypeScript left: their cases are the reference the Rust front is held to.
const { ice } = JSON.parse(readFileSync(file, "utf8"));

const vectors = {
  clean: clean.map((c) => ({
    ...c,
    out:
      c.fallback === undefined
        ? cleanCameraName(c.raw)
        : cleanCameraName(c.raw, c.fallback),
  })),
  unique: unique.map((u) => ({ ...u, out: uniqueCameraName(u.want, u.taken) })),
  match: match.map((m) => ({
    ...m,
    out: matchRemoteCamera(m.label, m.cameras),
  })),
  frame: frame.map((text) => ({ text, out: readCameraFrame(text) })),
  ice,
};

writeFileSync(file, `${JSON.stringify(vectors, null, 2)}\n`);
