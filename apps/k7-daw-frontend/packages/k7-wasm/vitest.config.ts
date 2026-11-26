// vitest.config.ts — the module's tests, over the disk.
// A reader would get wrong that this REPLACES the root config: the disk timeout is repeated.

import { defineConfig } from "vitest/config";
import { scaled } from "../../../../scripts/vitestScale.ts";

export default defineConfig({
  test: {
    exclude: ["**/node_modules/**"],
    testTimeout: scaled(30_000),
    hookTimeout: scaled(30_000),
  },
});
