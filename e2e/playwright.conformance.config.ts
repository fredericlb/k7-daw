// playwright.conformance.config.ts — the backend's HTTP contract, black box, no browser: run it against the
// TypeScript backend and against the Rust front before it, and both must pass (e2e/docs/conformance.md).
// Files run side by side, except a LANE's, which share a state no name of theirs keeps apart and run one at a time.
import { readdirSync } from "node:fs";
import { defineConfig } from "@playwright/test";

/** Each lane is a project of one worker, the longest first; the lanes and the rest run beside one another (docs/conformance.md). */
const LANES: Record<string, string[]> = {
  // One file service per account, and the account's served project: two of these at once cut each other's calls.
  service: [
    "serviceGolden",
    "serviceDocs",
    "serviceFiles",
    "serviceHistory",
    "serviceLive",
    "serviceMedia",
    "serviceMediaVerbs",
    "serviceListings",
    "serviceModules",
    "serviceChains",
    "serviceProjects",
    "serviceRenders",
    "serviceWorkspaces",
  ],
  // The engine registry and the session's engine binding: a registered or claimed engine is every spec's to see.
  engines: ["engines", "engineJobs", "serviceEngine", "localEngine"],
};

// A device has one account, so every spec that holds its file service or its served project shares it: one lane.
const own = readdirSync("./specs/conformance")
  .filter((f) => /^(service|engineLink|files)[A-Za-z]*\.spec\.ts$/.test(f))
  .map((f) => f.replace(/\.spec\.ts$/, ""));
for (const lane of Object.keys(LANES))
  LANES[lane] = LANES[lane].filter((n) => !own.includes(n));
LANES.service = [...LANES.service, ...own];

const file = (names: string[]) =>
  new RegExp(`[\\\\/](${names.join("|")})\\.spec\\.ts$`);
const laned = Object.values(LANES).flat();

export default defineConfig({
  testDir: "./specs/conformance",
  outputDir: "./test-results/conformance",
  // A dev machine's share: the backend under test runs on it too. CONFORMANCE_WORKERS=1 is the serial run.
  workers: Number(process.env.CONFORMANCE_WORKERS) || 4,
  retries: 0,
  // Tier 4 (remote engines, remote cameras) is deferred: its specs run only with E2E_TIER4=1 (docs/conformance.md).
  grepInvert: process.env.E2E_TIER4 ? undefined : /@tier4/,
  reporter: [["list"]],
  use: {
    baseURL: process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789",
    ignoreHTTPSErrors: true,
  },
  projects: [
    ...Object.entries(LANES).map(([name, names]) => ({
      name,
      testMatch: file(names),
      workers: 1,
    })),
    { name: "parallel", testIgnore: file(laned) },
  ],
});
