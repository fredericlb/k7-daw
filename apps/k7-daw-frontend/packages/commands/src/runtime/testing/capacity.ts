// capacity.ts — the song crate's capacity project (cap128), written to a folder: 128 lanes of 200 events, 16 plugins a lane, 16 buses, 3,072 automations.
// What a reader gets wrong: it is a port of `capacity_files()` in apps/k7-daw-wasm/song/tests/all/doc_support/world.rs; change them together.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const AMP = 'plugin({ type: "internal", name: "Chorus" })\n';
const lane = (extra: string): string => `extends("@meta/track")\n${extra}`;

/** The project's files by their path below the project folder; `lanes` is 128 for cap128, 32 for cap32. */
export function capacityFiles(lanes = 128): [string, string][] {
  const files: [string, string][] = [["project.k7x", ""]];
  const range = (n: number): number[] => Array.from({ length: n }, (_, i) => i);
  const notes = range(16).map((k) => `t(${k}).d(1).k(${40 + k})`);
  for (const s of range(8))
    files.push([`sequences/s${s}.k7x`, `notes([${notes.join(", ")}])\n`]);
  const drawn = range(16).map(
    (i) => `time("${1 + i * 2}:1:1").value(${i % 2})`,
  );
  files.push([
    "curves/c.k7x",
    `type("custom")\npoints([${drawn.join(", ")}])\n`,
  ]);
  const params = range(60).map((i) => `p${i}: ${i}`);
  const events = range(200).map(
    (e) => `{ ref: "s${e % 8}", start: "${1 + e}:1" }`,
  );
  const pairs: [string, string][] = [];
  for (const b of range(16)) {
    files.push([`nodes/bus${b}.k7x`, AMP]);
    pairs.push([`bus${b}`, '"MIX"']);
  }
  for (const l of range(lanes)) {
    files.push([`nodes/l${l}.k7x`, lane(`events([${events.join(", ")}])\n`)]);
    const steps: string[] = [];
    for (const p of range(16)) {
      const vst = l < lanes / 2 && p === 15;
      const plugin = vst ? 'vst3", name: "X' : 'internal", name: "Chorus';
      const autos =
        p < 8
          ? 'automations([{ param: "p1", curve: "c" }, { param: "p2", curve: "c" }, { param: "p3", curve: "c" }])\n'
          : "";
      files.push([
        `nodes/p${l}_${p}.k7x`,
        `plugin({ type: "${plugin}" })\nparams({ ${params.join(", ")} })\n${autos}`,
      ]);
      steps.push(`"p${l}_${p}"`);
    }
    pairs.push([`l${l}`, steps[0] as string]);
    for (const p of range(15))
      pairs.push([`p${l}_${p}`, steps[p + 1] as string]);
    pairs.push([`p${l}_15`, `"bus${l % 16}"`]);
  }
  files.push([
    "graph.k7x",
    `modules({ ${pairs.map(([k, v]) => `"${k}": [${v}]`).join(", ")} })\n`,
  ]);
  return files;
}

/** Writes the project at `dir`. */
export function writeCapacity(dir: string, lanes = 128): void {
  for (const [rel, text] of capacityFiles(lanes)) {
    const p = join(dir, rel);
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, text);
  }
}
