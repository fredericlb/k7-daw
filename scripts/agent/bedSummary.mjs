#!/usr/bin/env node
// bedSummary.mjs — what a bed run says about itself: `<e2e-out> [--ref <e2e-out>]`, the suites' lines and failed specs.
// What a reader gets wrong: it exits 0 whatever the suites did; the bed's status is the scripts' and bedCounters.mjs'.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { diff } from "./sameFailures.mjs";

/** Every failed test of a Playwright report with the first line of its first error. */
export function failedSpecs(report) {
  const out = [];
  const walk = (suite, titles) => {
    const here = suite.title ? [...titles, suite.title] : titles;
    for (const spec of suite.specs ?? [])
      for (const t of spec.tests ?? []) {
        const results = t.results ?? [];
        const last = results.at(-1)?.status ?? t.status;
        if (
          last !== "failed" &&
          last !== "timedOut" &&
          t.status !== "unexpected"
        )
          continue;
        const err = results.at(-1)?.error?.message ?? "";
        const first =
          err
            // biome-ignore lint/suspicious/noControlCharactersInRegex: strips ANSI colours
            .replace(/\u001b\[[0-9;]*m/g, "")
            .split("\n")
            .find((l) => l.trim()) ?? "(no error message)";
        out.push({
          name: [t.projectName, ...here, spec.title]
            .filter(Boolean)
            .join(" › "),
          error: first.trim(),
        });
      }
    for (const s of suite.suites ?? []) walk(s, here);
  };
  for (const s of report.suites ?? []) walk(s, []);
  return out;
}

const read = (path) => (existsSync(path) ? readFileSync(path, "utf8") : "");

/** The summary text of an e2e-out folder; `ref` is another one to compare the failures with. */
export function summarize(dir, ref, uptime) {
  const lines = [`=== bed summary (${dir}) ===`];
  const summary = read(join(dir, "summary.txt")).split("\n");
  for (const suite of ["web", "conformance"]) {
    const line = summary.find((l) => l.startsWith(`${suite}:`));
    lines.push(line ?? `${suite}: no result`);
  }
  for (const suite of ["web", "conformance"]) {
    const file = join(dir, `${suite}.json`);
    if (!existsSync(file)) continue;
    const failed = failedSpecs(JSON.parse(readFileSync(file, "utf8")));
    lines.push(`failed ${suite} specs: ${failed.length}`);
    for (const f of failed) lines.push(`  ${f.name}\n    ${f.error}`);
  }
  if (ref) {
    for (const suite of ["web", "conformance"]) {
      const a = join(ref, `${suite}.json`);
      const b = join(dir, `${suite}.json`);
      if (!existsSync(a) || !existsSync(b)) continue;
      const d = diff(
        JSON.parse(readFileSync(a, "utf8")),
        JSON.parse(readFileSync(b, "utf8")),
      );
      lines.push(
        `vs reference ${suite}: ${d.both} fail in both, ${d.onlyFirst.length} only in the reference, ${d.onlySecond.length} only here`,
      );
      for (const t of d.onlySecond) lines.push(`  new failure: ${t}`);
    }
  }
  lines.push(`uptime: ${uptime}`);
  return lines.join("\n");
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [, , first, second, third] = process.argv;
  if (!first) {
    console.error("usage: bedSummary.mjs <e2e-out> [--ref <e2e-out>]");
    process.exit(2);
  }
  const ref = second === "--ref" ? third : process.env.K7_BED_REF || undefined;
  console.log(
    summarize(first, ref, execFileSync("uptime", { encoding: "utf8" }).trim()),
  );
}
