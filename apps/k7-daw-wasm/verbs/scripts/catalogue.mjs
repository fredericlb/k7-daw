// catalogue.mjs — writes commands.json, the crate's copy of commands.yaml: each command's words, SID, texts, menus and arguments.
// What a reader gets wrong: `--check` only compares, so a catalogue change fails it until this is run without the flag.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "yaml";

const YAML = join(import.meta.dirname, "..", "commands.yaml");
const OUT = join(import.meta.dirname, "..", "commands.json");

const rows = (parse(readFileSync(YAML, "utf8")) ?? [])
  .filter((c) => c?.command && c?.sid)
  .map((c) => ({
    command: String(c.command),
    sid: String(c.sid),
    ...(c.help ? { help: String(c.help) } : {}),
    // Both languages or neither, so a French list never shows one English row in it.
    ...(c.readable?.en && c.readable?.fr
      ? { readable: { en: String(c.readable.en), fr: String(c.readable.fr) } }
      : {}),
    ...(Array.isArray(c.surfaces) ? { surfaces: c.surfaces.map(String) } : {}),
    ...(Array.isArray(c.menu) ? { menu: c.menu.map(String) } : {}),
    ...(c.confirm === true ? { confirm: true } : {}),
    args: (Array.isArray(c.args) ? c.args : []).map((a) => ({
      name: String(a?.name ?? ""),
      ...(a?.type !== undefined ? { type: String(a.type) } : {}),
      ...(a?.complete ? { complete: String(a.complete) } : {}),
      ...(Array.isArray(a?.values) ? { values: a.values.map(String) } : {}),
      ...(a?.hint ? { hint: String(a.hint) } : {}),
      ...(a?.optional === true ? { optional: true } : {}),
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
