// vitest.config.ts — the module's tests over the disk; edit.vfs.test.ts runs in vitest.vfs.config.ts, under the Vfs shims.
// A reader would get wrong that this REPLACES the root config: the disk timeout is repeated.

import { defineConfig } from "vitest/config";
import { scaled } from "../../../../scripts/vitestScale.ts";

export default defineConfig({
  test: {
    exclude: ["**/node_modules/**", "**/*.vfs.test.ts"],
    testTimeout: scaled(30_000),
    hookTimeout: scaled(30_000),
  },
});
