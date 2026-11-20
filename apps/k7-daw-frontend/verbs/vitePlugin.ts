// vitePlugin.ts — `virtual:k7-verbs-worker`, the URL of the verbs' Worker: served from memory in dev, an emitted hashed asset in a build.
// What a reader gets wrong: the dev URL is stable and uncached; a source change rebuilds on the next request, a build's names are immutable.

import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import { buildWorkerFiles, type WorkerFiles } from "./workerBuild.ts";

const ID = "virtual:k7-verbs-worker";
const RESOLVED = `\0${ID}`;
const DEV_DIR = "/verbs-worker/";
const BUILD_DIR = "assets/verbs/";
const PACKAGES = fileURLToPath(new URL("../../../packages/", import.meta.url));

export function verbsWorkerPlugin(): Plugin {
  let building = false;
  let base = "/";
  let built: Promise<WorkerFiles> | undefined;
  const files = (): Promise<WorkerFiles> => {
    built ??= buildWorkerFiles();
    return built;
  };
  return {
    name: "k7-verbs-worker",
    configResolved(config) {
      building = config.command === "build";
      base = config.base;
    },
    resolveId: (source) => (source === ID ? RESOLVED : undefined),
    async load(id) {
      if (id !== RESOLVED) return;
      const { entry } = await files();
      const url = building
        ? `${base}${BUILD_DIR}${entry}`
        : `${DEV_DIR}worker.js`;
      return `export default ${JSON.stringify(url)};`;
    },
    async generateBundle() {
      if (!building) return;
      for (const [name, bytes] of (await files()).files)
        this.emitFile({
          type: "asset",
          fileName: BUILD_DIR + name,
          source: bytes,
        });
    },
    configureServer(server) {
      // A package's source changing makes the next request build again.
      server.watcher.on("change", (file) => {
        if (file.startsWith(PACKAGES) && /\.(ts|tsx|yaml|mjs)$/.test(file))
          built = undefined;
      });
      server.middlewares.use(DEV_DIR, async (req, res, next) => {
        try {
          const { entry, files: all } = await files();
          const name = (req.url ?? "").split("?")[0]?.replace(/^\//, "") ?? "";
          const bytes = all.get(name === "worker.js" ? entry : name);
          if (!bytes) return next();
          res.setHeader("Content-Type", "text/javascript");
          res.setHeader("Cache-Control", "no-store");
          res.end(Buffer.from(bytes));
        } catch (e) {
          next(e);
        }
      });
    },
  };
}
