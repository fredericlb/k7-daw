// checkLayouts.ts — every library module's panel against the two-layout rules, the AI brief's checks, and what it offered BEFORE: a migrated panel must keep every control the
// committed one had, wherever it moved. The rules are Rust's (`panel.check_layout`); this walks the library, asks git for the committed text and compiles each widgets.js for its screens.
// Usage: tsx scripts/checkLayouts.ts [module dirs…] (default: all of k7-core). Exits 1 on any issue. See docs/layouts.md.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { checkLayout } from "@studio/k7-wasm/panel";

const LIBRARY = resolve(import.meta.dirname, "../../../../../k7-core");

/** Every ui.k7x under the library, or the ones of the module folders named. */
function targets(args: string[]): string[] {
  if (args.length) return args.map((a) => join(resolve(a), "ui.k7x"));
  const out = execFileSync("git", ["-C", LIBRARY, "ls-files", "*ui.k7x"], {
    encoding: "utf8",
  });
  return [
    ...new Set(
      [
        ...out
          .split("\n")
          .filter(Boolean)
          .map((f) => join(LIBRARY, f)),
        ...execFileSync("find", [join(LIBRARY, "modules"), "-name", "ui.k7x"], {
          encoding: "utf8",
        })
          .split("\n")
          .filter(Boolean),
        // One spelling per file, so git's and find's separators do not count a panel twice.
      ].map((f) => resolve(f)),
    ),
  ].filter((f) => existsSync(f));
}

/** The committed text of a panel, if the library's HEAD has one — what the migration must not lose. */
function committed(file: string): string | null {
  try {
    return execFileSync(
      "git",
      ["-C", LIBRARY, "show", `HEAD:${relative(LIBRARY, file)}`],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
  } catch {
    return null;
  }
}

/** The screen ids a widgets.js draws, compiled as the widget sandbox compiles it (ui-chains' widgetRuntime.ts): `export default` becomes a return. */
function screensOf(source: string): string[] {
  if (!source.trim()) return [];
  try {
    const out = new Function(
      `"use strict";\n${source.replace(/\bexport\s+default\b/, "return ")}`,
    )() as Record<string, unknown> | undefined;
    return Object.keys(out ?? {});
  } catch {
    return [];
  }
}

function check(file: string): string[] {
  const js = join(dirname(file), "widgets.js");
  const drawn = screensOf(existsSync(js) ? readFileSync(js, "utf8") : "");
  return checkLayout(readFileSync(file, "utf8"), committed(file), drawn);
}

const files = targets(process.argv.slice(2));
let bad = 0;
for (const f of files) {
  const issues = check(f);
  if (!issues.length) continue;
  bad++;
  console.log(`✗ ${relative(LIBRARY, dirname(f))}`);
  for (const i of issues) console.log(`    ${i}`);
}
console.log(`${files.length - bad}/${files.length} panels pass`);
process.exit(bad ? 1 : 0);
