// playwright.config.ts — the functional suite, run against a live instance (app.priv.k7.rocks by default).
// It drives a running dev session rather than starting one: `npm run dev` must already be up (docs/e2e.md).
import { defineConfig, devices } from "@playwright/test";
import { AUTH_STATE } from "./src/paths.ts";

const baseURL = process.env.E2E_BASE_URL ?? "https://app.priv.k7.rocks";

export default defineConfig({
  testDir: "./specs",
  outputDir: "./test-results",
  fullyParallel: false,
  // One worker: the specs share the e2e account's workspace and one engine.
  workers: 1,
  retries: 0,
  // Tier 4 (remote engines, the second device) is deferred: its specs run only with E2E_TIER4=1 (docs/e2e.md).
  grepInvert: process.env.E2E_TIER4 ? undefined : /@tier4/,
  reporter: [["list"], ["html", { outputFolder: "./report", open: "never" }]],
  // A dev server sends a page's modules one request each, and a cold page can take longer than the default 5 s to draw.
  expect: { timeout: 15_000 },
  use: {
    baseURL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    { name: "setup", testMatch: /auth\.setup\.ts/ },
    { name: "seed", testMatch: /seed\.setup\.ts/, dependencies: ["setup"] },
    {
      name: "web",
      testMatch: /specs[\\/]app[\\/].*\.spec\.ts/,
      dependencies: ["seed"],
      use: {
        ...devices["Desktop Chrome"],
        storageState: AUTH_STATE,
        // A fake camera and microphone, granted without a prompt: a picture lane records its webcam in the browser.
        launchOptions: {
          args: [
            "--use-fake-ui-for-media-stream",
            "--use-fake-device-for-media-stream",
            // The page plays nothing through the speakers of the machine running the suite.
            "--mute-audio",
          ],
        },
      },
    },
  ],
});
