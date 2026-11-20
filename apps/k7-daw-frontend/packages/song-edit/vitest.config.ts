// A developer running these tests has a real `$STUDIO_HOME`, where the account's libraries are
// cloned. It is pinned, once, at a folder that does not exist: modules are a submodule in each
// test's own temp workspace (see testSupport's writeLibraries).

import { tmpdir } from "node:os";
import { join } from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
  // A package config REPLACES the root one, so the disk timeout is repeated here — docs/tests.md.
  test: {
    env: {
      // The machine's state, where the account's LIBRARIES are cloned: a test that wants
      // one points it at its own folder (testSupport's `librariesFixture`).
      STUDIO_HOME: join(tmpdir(), "studio-song-no-home"),
    },
    testTimeout: 30_000,
    hookTimeout: 30_000,
  },
});
