// vitest.vfs.config.ts — the Worker runtime's tests and the translator's, with `node:fs` and friends the Vfs shims.
// A reader would get wrong that fixtures are written into the Vfs too: a test here never touches the disk.

import { join } from "node:path";
import { defineConfig } from "vitest/config";

const shim = (name: string): string =>
  join(import.meta.dirname, "src/runtime/shims", name);

export default defineConfig({
  resolve: {
    alias: [
      { find: /^node:fs$/, replacement: shim("fs.ts") },
      { find: /^node:fs\/promises$/, replacement: shim("fsPromises.ts") },
      { find: /^node:path$/, replacement: shim("path.ts") },
      { find: /^node:os$/, replacement: shim("os.ts") },
      { find: /^node:url$/, replacement: shim("url.ts") },
      { find: /^node:child_process$/, replacement: shim("childProcess.ts") },
    ],
  },
  test: {
    // The translator's own tests run here too: its emitter is the Worker's import, so it must hold over the shims (P9); emitNodes compiles with the node compiler and stays out.
    include: [
      "src/runtime/**/*.vfs.test.ts",
      "../translator/src/{emitGolden,ableton,abletonZip,dawproject,inflate}.test.ts",
    ],
    exclude: ["**/node_modules/**"],
    testTimeout: 30_000,
    hookTimeout: 30_000,
    env: {
      STUDIO_HOME: "/tmp/studio-cmd-no-home",
    },
  },
});
