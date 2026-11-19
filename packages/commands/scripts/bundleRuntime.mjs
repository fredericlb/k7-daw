// bundleRuntime.mjs — builds the verbs' Worker (browser) and the node runner with `node:fs` and friends aliased to the Vfs shims.
// What a reader gets wrong: a Worker build leaves other `node:*` imports external and LISTS them — each is a server-only import 3f must move behind a host member.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { build } from "esbuild";

const PKG = join(import.meta.dirname, "..");
const SHIMS = join(PKG, "src", "runtime", "shims");
const ALIASED = {
  "node:fs": "fs.ts",
  "node:fs/promises": "fsPromises.ts",
  "node:path": "path.ts",
  "node:os": "os.ts",
  "node:url": "url.ts",
  "node:child_process": "childProcess.ts",
};

// The modules a browser cannot run: the 25 server-only imports of the verbs and the transitive ones.
const SERVER_ONLY = [
  "shims/childProcess.ts",
  "engine-client/src/engineBinary",
  "engine-client/src/engineClient",
  "audio-passes/src/peakPass.ts",
  "take-render/src/renderer.ts",
  "take-render/src/wavLength.ts",
  "video-prep/src/probe.ts",
  "video-prep/src/transcode.ts",
  "audio-analysis/src/extract",
  "audio-analysis/src/pool",
  "k7-plugins/src/transcription/transcribe.ts",
  "ffmpeg/src/",
  "take-render/src/render.ts",
  "engine-tunnel/",
  "audio-analysis/src/decode",
];

/** The server-only modules a build's metafile still holds, by the path that names them (relative to the cwd, so only the package part is matched). */
export function serverInputs(metafile) {
  return Object.keys(metafile.inputs)
    .filter((path) => SERVER_ONLY.some((m) => path.includes(m)))
    .sort();
}

/** Builds one target into `outdir` (`entry`: another file under src/runtime); answers what a size report needs. */
export async function bundleRuntime({ target, outdir, entry }) {
  const leftover = new Set();
  const leftoverBy = {};
  const plugin = {
    name: "runtime-aliases",
    setup(b) {
      b.onResolve({ filter: /^node:/ }, (a) => {
        const shim = ALIASED[a.path];
        if (shim) return { path: join(SHIMS, shim) };
        if (a.importer.startsWith(SHIMS) && a.path === "node:fs")
          return undefined;
        if (target === "worker") {
          leftover.add(a.path);
          (leftoverBy[a.path] ??= []).push(a.importer);
        }
        return target === "worker"
          ? { path: a.path, external: true }
          : undefined;
      });
      b.onResolve({ filter: /\?raw$/ }, (a) => ({
        path: join(a.resolveDir, a.path.slice(0, -4)),
        namespace: "raw",
      }));
      b.onLoad({ filter: /.*/, namespace: "raw" }, (a) => ({
        contents: readFileSync(a.path, "utf8"),
        loader: "text",
      }));
    },
  };
  const worker = target === "worker";
  const result = await build({
    entryPoints: [
      join(PKG, "src", "runtime", entry ?? (worker ? "worker.ts" : "node.ts")),
    ],
    outdir,
    bundle: true,
    format: "esm",
    platform: worker ? "browser" : "node",
    splitting: worker,
    target: "es2022",
    minify: worker,
    metafile: true,
    write: true,
    logLevel: "error",
    outExtension: { ".js": worker ? ".js" : ".mjs" },
    plugins: [plugin],
    banner: worker
      ? {}
      : {
          js: "import { createRequire as __cr } from 'node:module'; const require = __cr(import.meta.url);",
        },
  });
  const files = Object.entries(result.metafile.outputs).map(([path, o]) => ({
    path,
    bytes: o.bytes,
    gzip: gzipSync(
      readFileSync(path.startsWith("/") ? path : join(process.cwd(), path)),
    ).length,
  }));
  const packages = {};
  for (const [path, i] of Object.entries(result.metafile.inputs)) {
    const m = /(?:packages\/([^/]+)|node_modules\/((?:@[^/]+\/)?[^/]+))\//.exec(
      path,
    );
    const key = m?.[1] ? `@studio/${m[1]}` : (m?.[2] ?? "other");
    packages[key] = (packages[key] ?? 0) + i.bytes;
  }
  return {
    files,
    packages,
    leftover: [...leftover].sort(),
    leftoverBy,
    serverLeft: serverInputs(result.metafile),
  };
}

if (process.argv[1] === import.meta.filename) {
  const target = process.argv[2] === "node" ? "node" : "worker";
  const outdir = process.argv[3] ?? join(PKG, "dist", `runtime-${target}`);
  const r = await bundleRuntime({ target, outdir });
  for (const f of r.files)
    console.log(`${f.path}: ${f.bytes} bytes, ${f.gzip} gzip`);
  console.log(
    `node builtins left external: ${r.leftover.join(", ") || "none"}`,
  );
  console.log(`server-only modules left: ${r.serverLeft.join(", ") || "none"}`);
  const top = Object.entries(r.packages)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 12);
  console.log(`largest inputs: ${top.map(([k, v]) => `${k} ${v}`).join(", ")}`);
  writeFileSync(join(outdir, "report.json"), JSON.stringify(r, null, 2));
  // A Worker that still holds a server-only module is no build: the check is the exit code.
  if (target === "worker" && r.serverLeft.length) {
    console.error(
      `the Worker holds server-only modules: ${r.serverLeft.join(", ")}`,
    );
    process.exit(1);
  }
}
