// catalogue.mjs — writes commands.json, the crate's copy of commands.yaml: each command's words, SID and argument shapes.
// What a reader gets wrong: `--check` only compares, so a catalogue change fails it until this is run without the flag.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "yaml";

const ROOT = join(import.meta.dirname, "..", "..", "..", "..");
const YAML = join(ROOT, "apps/k7-daw-frontend/packages/commands/commands.yaml");
const OUT = join(import.meta.dirname, "..", "commands.json");

const rows = (parse(readFileSync(YAML, "utf8")) ?? [])
  .filter((c) => c?.command && c?.sid)
  .map((c) => ({
    command: String(c.command),
    sid: String(c.sid),
    args: (Array.isArray(c.args) ? c.args : []).map((a) => ({
      name: String(a?.name ?? ""),
      ...(a?.type ? { type: String(a.type) } : {}),
    })),
  }));
const text = `${JSON.stringify(rows, null, 1)}\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(OUT, "utf8") !== text) {
    console.error(
      "verbs/commands.json is stale: run node apps/k7-daw-wasm/verbs/scripts/catalogue.mjs",
    );
    process.exit(1);
  }
} else writeFileSync(OUT, text);
