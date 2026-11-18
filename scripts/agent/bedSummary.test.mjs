// bedSummary.test.mjs — the summary carries what an agent would otherwise dig for.
// Run: node --test scripts/agent/bedSummary.test.mjs

import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { failedSpecs, summarize } from "./bedSummary.mjs";

const report = {
  suites: [
    {
      title: "a.spec.ts",
      specs: [
        {
          title: "breaks",
          tests: [
            {
              projectName: "chromium",
              status: "unexpected",
              results: [
                {
                  status: "failed",
                  error: { message: "\u001b[31mExpected 1\u001b[0m\nmore" },
                },
              ],
            },
          ],
        },
        {
          title: "fine",
          tests: [{ status: "expected", results: [{ status: "passed" }] }],
        },
      ],
    },
  ],
};

test("failed specs carry their first error line", () => {
  const f = failedSpecs(report);
  assert.equal(f.length, 1);
  assert.equal(f[0].error, "Expected 1");
});

test("the summary has the counts, the failures, the reference and uptime", () => {
  const dir = mkdtempSync(join(tmpdir(), "bedsum-"));
  writeFileSync(
    join(dir, "summary.txt"),
    "web: 3 passed, 1 failed, 0 flaky, 0 skipped, 9 s\n",
  );
  writeFileSync(join(dir, "web.json"), JSON.stringify(report));
  const out = summarize(dir, dir, "up 1 day");
  assert.match(out, /^web: 3 passed/m);
  assert.match(out, /^conformance: no result/m);
  assert.match(out, /breaks\n {4}Expected 1/);
  assert.match(out, /vs reference web: 1 fail in both/);
  assert.match(out, /uptime: up 1 day/);
});
