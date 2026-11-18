// graphTwinCheck.ts — the type test of decision 438: each type ts-rs derives is assignable to the hand-written one the frontend reads.
//
//   npx tsx scripts/graphTwinCheck.ts <dir>     (from packages/song; `cargo test --features ts --test all -- --ignored graph_twin::` wrote <dir>/twin.ts)
//
// What a reader gets wrong: the direction is generated -> hand-written (what the route gives is what the frontend reads), and the keys
// of each pair are equal up to the KNOWN_ABSENT list, each entry with its reason. One @ts-expect-error proves the check can fail.

import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const dir = process.argv[2];
if (!dir) throw new Error("usage: graphTwinCheck.ts <dir>");
const here = dirname(fileURLToPath(import.meta.url));
const model = resolve(here, "../../graph-model/types.ts");

/** Hand-written type expressions and the keys they carry that the generated type knows not, with why. */
const PAIRS: [name: string, hand: string, absent: string[]][] = [
  ["WebOut", "H.WebOut", []],
  ["WebPortIn", 'NonNullable<H.WebChainStep["inputs"]>[number]', []],
  ["WebPortOut", 'NonNullable<H.WebChainStep["outputs"]>[number]', []],
  ["WebModelDownload", 'NonNullable<H.WebChainStep["modelDownload"]>', []],
  ["WebAmpModel", 'NonNullable<H.WebChainStep["ampModel"]>', []],
  ["WebRackSound", "H.WebRackSound", []],
  ["WebAutomation", "H.WebAutomation", []],
  ["WebAltChoice", 'NonNullable<H.WebGraph["trackAlternatives"]>[string]', []],
  ["WebDocNote", 'NonNullable<H.WebGraph["modules"]>[string]', []],
  ["WebTrackClip", "H.WebTrackClip", []],
  ["WebPlug", 'NonNullable<H.WebNodeAssembly["inputs"]>[number]', []],
  ["WebAssemblyParam", 'NonNullable<H.WebNodeAssembly["params"]>[string]', []],
  ["WebNodeAssembly", "H.WebNodeAssembly", []],
  ["WebCurve", "H.WebCurve", []],
  ["WebChainStep", "H.WebChainStep", []],
  // `enabled` is the whole-chain bypass: nothing sets it (decision 439).
  ["WebChain", "H.WebChain", ["enabled"]],
  ["WebGraph", "H.WebGraph", []],
  ["GraphIssue", "H.GraphIssue", []],
];

const twin = readFileSync(join(dir, "twin.ts"), "utf8");
// The leaf types the generated file names and the derive cannot spell: `ui`, `cc`, the category, the signal and the curve's definition.
const prelude =
  'import type { WebModuleCategory, WebModuleUI, WebCc, WebSignal, CurveSpec } from "@studio/graph-model";\n';
writeFileSync(join(dir, "twin_full.ts"), prelude + twin);

const lines = [
  'import type * as R from "./twin_full.ts";',
  'import type * as H from "@studio/graph-model";',
  "type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;",
  "declare const assign: <T>(v: T) => void;",
];
for (const [name, hand, absent] of PAIRS) {
  lines.push(`declare const r${name}: R.${name};`);
  lines.push(`assign<${hand}>(r${name});`);
  const left = `keyof R.${name}`;
  const right = absent.length
    ? `Exclude<keyof ${hand}, ${absent.map((k) => JSON.stringify(k)).join(" | ")}>`
    : `keyof ${hand}`;
  lines.push(`export const keys${name}: Same<${left}, ${right}> = true;`);
}
// A warning code can be neither missing nor extra.
lines.push("declare const hCode: H.GraphWarningCode;");
lines.push("assign<R.WarningCode>(hCode);");
lines.push("declare const rCode: R.WarningCode;");
lines.push("assign<H.GraphWarningCode>(rCode);");
// The check can fail: a wrong field is refused, or the directive below is reported unused.
lines.push("// @ts-expect-error a `gain` that is a string is no WebOut");
lines.push(
  "assign<H.WebOut>(null as unknown as { to: string; gain: string });",
);
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
      paths: { "@studio/graph-model": [model] },
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
  `the twin check passed: ${PAIRS.length} pairs and the warning codes both ways`,
);
