// viewsTwinCheck.ts — the type test of decision 644: each type ts-rs derives for a view is assignable to the one the frontend reads.
//
//   npx tsx scripts/viewsTwinCheck.ts <dir>     (from packages/song; `cargo test --features ts --test all -- --ignored views_twin::` wrote <dir>/twin.ts)
//
// What a reader gets wrong: the direction is generated -> hand-written (what the route gives is what the frontend reads), and the keys
// of each pair are equal up to the two named lists, each entry with its reason. A wrong field proves the check can fail.

import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const dir = process.argv[2];
if (!dir) throw new Error("usage: viewsTwinCheck.ts <dir>");
const here = dirname(fileURLToPath(import.meta.url));

type Pair = {
  name: string;
  hand: string;
  /** Keys the hand-written type has and the Rust never writes. */
  absent?: string[];
  /** Keys the Rust writes and the hand-written type does not list. */
  extra?: string[];
};

// The hand-written types are named by absolute path: the check lives in a folder outside the workspace.
const at = (rel: string) => `import(${JSON.stringify(resolve(here, rel))})`;
const WP = at("../../web-protocol/types.ts");
const SF = at("../../song-schema/src/yaml/surfaceFile.ts");
const SS = at("../../song-schema/src/yaml/surfaceSchema.ts");
// moduleRefs.ts imports half of song, so its one interface is taken from the text.
const moduleRefs = readFileSync(resolve(here, "../src/moduleRefs.ts"), "utf8");
const baseRef = /export interface BaseModuleRef \{[\s\S]*?\n\}/.exec(
  moduleRefs,
)?.[0];
if (!baseRef) throw new Error("BaseModuleRef is not in moduleRefs.ts");
const MR = "HandBase";
const WT = `${WP}.WebTrack`;
// A control's binding as the surface file's own schema types it: its key and the `also` it may hold.
const BINDING = `NonNullable<NonNullable<${SS}.Surface["mapping"]>[string]>`;
const HINTS = `${WP}.Hints`;

const PAIRS: Pair[] = [
  { name: "Diagnostic", hand: `${WP}.Diagnostic` },
  { name: "FormatTooNew", hand: `${WP}.FormatTooNew` },
  { name: "ProjectErrors", hand: `${WP}.ProjectErrors` },
  { name: "WebNote", hand: `${WP}.WebClip["notes"][number]` },
  {
    name: "WebTimelineRef",
    hand: `NonNullable<${WP}.WebClip["timeline"]>`,
  },
  // `event`, `comp` and `crossfade` are typed by hand in the derive (ts(type = …)), so they rest on the gate's run-time comparison.
  { name: "WebClip", hand: `${WP}.WebClip` },
  { name: "WebMixer", hand: `${WP}.WebMixer` },
  { name: "WebPlugin", hand: `${WT}["plugins"][number]` },
  // `disabled` is never set: no field of the document says it (decision 645).
  { name: "WebTrack", hand: WT, absent: ["disabled"] },
  { name: "WebGroup", hand: `${WP}.WebGroup` },
  { name: "WebMarker", hand: `${WP}.WebMarker` },
  {
    name: "WebTimeline",
    hand: `NonNullable<${WP}.WebProject["timelines"]>[number]`,
  },
  { name: "WebProject", hand: `${WP}.WebProject` },
  { name: "SurfaceType", hand: `${SS}.SurfaceType` },
  { name: "SurfaceLayout", hand: `${SS}.SurfaceLayout` },
  { name: "BindTarget", hand: `NonNullable<${BINDING}["also"]>` },
  { name: "ViewBinding", hand: BINDING },
  { name: "SurfaceFile", hand: `${SS}.Surface` },
  { name: "ProjectSurface", hand: `${SF}.ProjectSurface` },
  { name: "ProjectSurfaces", hand: `${SF}.ProjectSurfaces` },
  { name: "Panel", hand: `NonNullable<${MR}["panel"]>` },
  { name: "CatalogueModule", hand: `${WP}.CatalogueModule` },
  { name: "BaseModuleRef", hand: MR },
  { name: "IoPort", hand: `${WP}.IoPort` },
  { name: "ParamRange", hand: `${HINTS}["paramMetaByBase"][string][string]` },
  { name: "RackSymbol", hand: `${HINTS}["racks"][string]["symbols"][number]` },
  { name: "RackHint", hand: `${HINTS}["racks"][string]` },
  // The route answers the three lists the editor's type does not name (decision 645).
  {
    name: "Hints",
    hand: HINTS,
    extra: ["audioClips", "videoClips", "curves"],
  },
];

const names = (keys: string[]) =>
  keys.map((k) => JSON.stringify(k)).join(" | ");
const twin = readFileSync(join(dir, "twin.ts"), "utf8");
// The leaf the derive names and cannot spell: an event as the timeline's clip carries it.
const prelude = `import type { WebEvent } from ${JSON.stringify(resolve(here, "../../web-protocol/types.ts"))};\n`;
writeFileSync(join(dir, "twin_full.ts"), prelude + twin);

const lines = [
  'import type * as R from "./twin_full.ts";',
  "type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;",
  "declare const assign: <T>(v: T) => void;",
  baseRef.replace("export interface BaseModuleRef", `interface ${MR}`),
];
for (const { name, hand, absent = [], extra = [] } of PAIRS) {
  lines.push(`declare const r${name}: R.${name};`);
  lines.push(`assign<${hand}>(r${name});`);
  const left = extra.length
    ? `Exclude<keyof R.${name}, ${names(extra)}>`
    : `keyof R.${name}`;
  const right = absent.length
    ? `Exclude<keyof ${hand}, ${names(absent)}>`
    : `keyof ${hand}`;
  lines.push(`export const keys${name}: Same<${left}, ${right}> = true;`);
}
// Two enums that are the same on both sides.
for (const [r, h] of [
  ["SurfaceType", `${SS}.SurfaceType`],
  ["SurfaceLayout", `${SS}.SurfaceLayout`],
  ["IoDir", `${WP}.IoPort["dir"]`],
] as const) {
  lines.push(`declare const h${r}: ${h};`);
  lines.push(`assign<R.${r}>(h${r});`);
}
// The check can fail: a wrong field is refused, or the directive below is reported unused.
lines.push("// @ts-expect-error a `volume` that is a string is no WebMixer");
lines.push(
  `assign<${WP}.WebMixer>(null as unknown as { module: string; volume: string; pan: number });`,
);
lines.push("// @ts-expect-error a missing `plugins` is no WebTrack");
lines.push(`assign<${WT}>(null as unknown as Omit<R.WebTrack, "plugins">);`);
writeFileSync(join(dir, "check.ts"), `${lines.join("\n")}\n`);
writeFileSync(
  join(dir, "tsconfig.json"),
  JSON.stringify({
    compilerOptions: {
      strict: true,
      noEmit: true,
      target: "es2022",
      module: "esnext",
      moduleResolution: "bundler",
      allowImportingTsExtensions: true,
      skipLibCheck: true,
      types: [],
    },
    files: ["check.ts"],
  }),
);
const run = spawnSync("npx", ["tsc", "-p", join(dir, "tsconfig.json")], {
  cwd: resolve(here, ".."),
  encoding: "utf8",
});
process.stdout.write(run.stdout ?? "");
process.stderr.write(run.stderr ?? "");
if (run.status !== 0) {
  console.error("the twin check FAILED");
  process.exit(1);
}
console.log(
  `the twin check passed: ${PAIRS.length} pairs, three enums both ways and two refusals`,
);
