// Same pin as @studio/song's: a developer running these tests has a real `$STUDIO_HOME`. The
// picture compile goes through the song loader, so it is pinned, once, at a folder that does not exist.

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
