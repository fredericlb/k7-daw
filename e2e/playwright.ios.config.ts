// playwright.ios.config.ts — the same specs, against the app on the iOS simulator: its studio, driven in the host's WebKit.
// The app must be built (engine/k7-runtime/docs/ios.md); the target relaunches it on its own port (docs/e2e.md).
import { defineConfig } from "@playwright/test";
import { IOS_ORIGIN } from "./src/ios.ts";

// Read by src/target.ts in every worker, which inherits the runner's environment.
process.env.E2E_TARGET = "ios";
// The verb runner is node's and wants a whole URL: the app's own studio, not the web target's default.
process.env.E2E_BASE_URL = IOS_ORIGIN;

export default defineConfig({
  testDir: "./specs",
  testMatch: /specs[\\/]app[\\/].*\.spec\.ts/,
  outputDir: "./test-results/ios",
  fullyParallel: false,
  // One worker: one simulator, one engine, one app.
  workers: 1,
  retries: 0,
  timeout: 120_000,
  reporter: [
    ["list"],
    ["html", { outputFolder: "./report/ios", open: "never" }],
  ],
  use: { baseURL: IOS_ORIGIN, trace: "retain-on-failure" },
  projects: [{ name: "ios" }],
});
