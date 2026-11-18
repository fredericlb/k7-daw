// vitest.vfs.config.ts — the module's edits over the Vfs: `node:fs` and friends are the Worker's shims, as in the commands package.
// A reader would get wrong that the shims live in `commands`: this package borrows them to prove the same imports serve both.

import { join } from "node:path";
import { defineConfig } from "vitest/config";

const shim = (name: string): string =>
  join(import.meta.dirname, "../commands/src/runtime/shims", name);

export default defineConfig({
  resolve: {
    alias: [
      { find: /^node:fs$/, replacement: shim("fs.ts") },
      { find: /^node:path$/, replacement: shim("path.ts") },
      { find: /^node:os$/, replacement: shim("os.ts") },
    ],
  },
  test: {
    include: ["src/**/*.vfs.test.ts"],
    testTimeout: 30_000,
    hookTimeout: 30_000,
  },
});
