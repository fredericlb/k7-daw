// vitest.config.ts — the module's tests over the real corpora, whose parse starves under a loaded machine.

import { defineConfig } from "vitest/config";
import { scaled } from "../../scripts/vitestScale.ts";

export default defineConfig({
  test: { testTimeout: scaled(30_000), hookTimeout: scaled(30_000) },
});
