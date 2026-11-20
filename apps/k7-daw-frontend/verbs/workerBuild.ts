// workerBuild.ts — the verbs' Worker as files: packages/commands' own bundle plus stand-ins for the node builtins song still imports.
// What a reader gets wrong: the bundle is the artefact the runner tests prove; only the two leftover `node:` specifiers are rewritten.

import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../../../packages/commands/scripts/bundleRuntime.mjs";

const SHIMS = fileURLToPath(new URL("./shims/", import.meta.url));
const STAND_INS = {
  "node:crypto": "crypto",
  "node:worker_threads": "worker_threads",
};

export interface WorkerFiles {
  /** The entry's file name (hashed) and every file by name. */
  entry: string;
  files: Map<string, Uint8Array>;
}

const hashed = (name: string, bytes: Uint8Array): string => {
  const h = createHash("sha256").update(bytes).digest("hex").slice(0, 8);
  return name.replace(/\.js$/, `-${h}.js`);
};

/** Builds the Worker; the files are named so a name never means two contents. */
export async function buildWorkerFiles(
  entryFile = "worker.ts",
): Promise<WorkerFiles> {
  const out = mkdtempSync(join(tmpdir(), "k7-verbs-"));
  try {
    await bundleRuntime({
      target: "worker",
      outdir: join(out, "rt"),
      entry: entryFile,
    });
    await build({
      entryPoints: Object.fromEntries(
        Object.values(STAND_INS).map((n) => [n, join(SHIMS, `${n}.ts`)]),
      ),
      outdir: join(out, "shims"),
      bundle: true,
      format: "esm",
      platform: "browser",
      target: "es2022",
      minify: true,
      logLevel: "error",
    });
    const files = new Map<string, Uint8Array>();
    const names: Record<string, string> = {};
    for (const [spec, n] of Object.entries(STAND_INS)) {
      const bytes = readFileSync(join(out, "shims", `${n}.js`));
      const name = hashed(`shim-${n}.js`, bytes);
      files.set(name, bytes);
      names[spec] = `./${name}`;
    }
    let entry = "";
    for (const name of readdirSync(join(out, "rt")).filter((f) =>
      f.endsWith(".js"),
    )) {
      let text = readFileSync(join(out, "rt", name), "utf8");
      for (const [spec, to] of Object.entries(names))
        text = text.replaceAll(`"${spec}"`, `"${to}"`);
      const bytes = new TextEncoder().encode(text);
      // A chunk's name is already its hash; only the entry is renamed.
      const isEntry =
        name === entryFile.replace(/^.*\//, "").replace(/\.ts$/, ".js");
      const final = isEntry ? hashed(name, bytes) : name;
      if (isEntry) entry = final;
      files.set(final, bytes);
    }
    if (!entry) throw new Error(`the Worker build produced no ${entryFile}`);
    return { entry, files };
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}
