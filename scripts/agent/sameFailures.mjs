#!/usr/bin/env node
// sameFailures.mjs <off.json> <on.json> — two Playwright JSON reports must fail the same tests: the switch's `on` changes no outcome.
// What a reader gets wrong: a test that fails in both is not a regression, and one that fails in only one is, whichever one.

import { readFileSync } from "node:fs";

/** The tests a report failed, each by its project, file and title path. */
export function failures(report) {
  const out = new Set();
  const walk = (suite, titles) => {
    const here = suite.title ? [...titles, suite.title] : titles;
    for (const spec of suite.specs ?? [])
      for (const t of spec.tests ?? []) {
        const last = t.results?.at(-1)?.status ?? t.status;
        if (
          last === "failed" ||
          last === "timedOut" ||
          t.status === "unexpected"
        )
          out.add(
            `${t.projectName ?? ""} › ${[...here, spec.title].join(" › ")}`,
          );
      }
    for (const s of suite.suites ?? []) walk(s, here);
  };
  for (const s of report.suites ?? []) walk(s, []);
  return out;
}

/** What differs between two reports: failed only by the first, and only by the second. */
export function diff(a, b) {
  const fa = failures(a);
  const fb = failures(b);
  return {
    onlyFirst: [...fa].filter((t) => !fb.has(t)),
    onlySecond: [...fb].filter((t) => !fa.has(t)),
    both: [...fa].filter((t) => fb.has(t)).length,
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [, , first, second] = process.argv;
  if (!first || !second) {
    console.error("usage: sameFailures.mjs <off.json> <on.json>");
    process.exit(2);
  }
  const d = diff(
    JSON.parse(readFileSync(first, "utf8")),
    JSON.parse(readFileSync(second, "utf8")),
  );
  console.log(`sameFailures: ${d.both} fail in both`);
  for (const t of d.onlyFirst) console.error(`only in ${first}: ${t}`);
  for (const t of d.onlySecond) console.error(`only in ${second}: ${t}`);
  process.exit(d.onlyFirst.length || d.onlySecond.length ? 1 : 0);
}
