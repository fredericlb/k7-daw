// generated-check.mjs — fails when a tracked generated file is not what its sources build, naming the file and the command.
// It never writes: the WebAssembly modules are compared by their source hashes.
// What a reader gets wrong: a hash match says the Rust sources are unchanged, not that the module was rebuilt on this machine.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = join(import.meta.dirname, "..", "..");
const stale = [];

/** The `SOURCES`/`TYPES` constant a generated module carries. */
const constant = (text, name) =>
  text.match(new RegExp(`export const ${name} = "([0-9a-f]+)"`))?.[1];

async function wasm(file, pkg, withTypes, sources = "SOURCES") {
  const mod = await import(
    pathToFileURL(
      join(
        root,
        "apps",
        "k7-daw-frontend",
        "packages",
        pkg,
        "scripts",
        "rustSources.mjs",
      ),
    )
  );
  const path = join("apps", "k7-daw-frontend", "packages", pkg, "src", file);
  const text = readFileSync(join(root, path), "utf8");
  const bad =
    constant(text, sources) !== mod.rustSourceHash() ||
    (withTypes && constant(text, "TYPES") !== mod.typesHash());
  if (bad) stale.push(path);
}

await wasm("k7Wasm.generated.ts", "k7-wasm", true);
{
  const mod = await import(
    pathToFileURL(
      join(
        root,
        "apps",
        "k7-daw-frontend",
        "packages",
        "audio-analysis",
        "scripts",
        "rustSources.mjs",
      ),
    )
  );
  const stamp = readFileSync(
    join(
      root,
      "apps",
      "k7-daw-frontend",
      "packages",
      "audio-analysis",
      "waveform.wasm.sources",
    ),
    "utf8",
  ).trim();
  if (stamp !== mod.rustSourceHash())
    stale.push("apps/k7-daw-frontend/packages/audio-analysis/waveform.wasm");
}
{
  const mod = await import(
    pathToFileURL(
      join(root, "apps", "k7-daw-backend", "scripts", "bindingsSources.mjs"),
    )
  );
  for (const target of mod.TARGETS)
    if (mod.stampOf(target) !== mod.bindingsHash())
      stale.push(
        join("apps", "k7-daw-frontend", "packages", target, "generated"),
      );
}
if (stale.length > 0) {
  console.error(`generated-check: stale:\n  ${stale.join("\n  ")}`);
  console.error(
    "generated-check: run `npm run rebuild:generated` and commit what it writes",
  );
  process.exit(1);
}
console.log("generated-check: clean");
