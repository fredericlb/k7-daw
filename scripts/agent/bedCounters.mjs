#!/usr/bin/env node
// bedCounters.mjs — judges a bed's run from the native counters the front logs: the engine link, the jobs, media,
// the shares, the runtime line and the shed. A panic fails it, and so does a counter under its floor, which would read as green.
// What a reader gets wrong: each line is cumulative, so the LAST one counts (per account where the line names one, summed).
// The engine's poll floor is a rate over the web suite's length, not a count: the browser polls on a wall clock (e2e/docs/e2e.md).
// Usage: node scripts/agent/bedCounters.mjs [--info] <e2e-out> <web|conformance|both>
//   --info: a floor missed is information (a filtered run); a panic still fails.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** Each `key=N` of a line from `from` on, as numbers. */
function counts(line, from = 0) {
  const out = {};
  for (const [, key, n] of line.slice(from).matchAll(/(\w+)=(\d+)/g))
    out[key] = Number(n);
  return out;
}

/** The last line holding `marker`, or undefined. */
const lastLine = (text, marker) =>
  text
    .split("\n")
    .filter((l) => l.includes(marker))
    .at(-1);

/** The accounts' last `<marker> <account>:` lines summed, or null when none was logged; panics is the largest seen on any line. */
function perAccount(text, marker) {
  const last = new Map();
  const re = new RegExp(`${marker} (\\S+): `);
  let panics = 0;
  for (const l of text.split("\n")) {
    const m = l.match(re);
    if (!m) continue;
    last.set(m[1], l);
    for (const [, n] of l.matchAll(/panics=(\d+)/g))
      panics = Math.max(panics, Number(n));
  }
  if (!last.size) return null;
  const sum = {};
  for (const l of last.values())
    for (const [key, n] of Object.entries(counts(l, l.indexOf(marker))))
      sum[key] = (sum[key] ?? 0) + n;
  sum.panics = panics;
  return sum;
}

/** Floors under which `r` fails, pushed to `info` instead when one is given. */
function floors(r, table, why, info) {
  for (const [key, floor] of Object.entries(table))
    if ((r[key] ?? 0) < floor)
      (info ?? why).push(
        `${key}=${r[key] ?? 0} is under its floor of ${floor}`,
      );
}

// What the jobs must have answered in a run of the conformance suite (2362); freezes and separations have no floor: no suite reaches them.
const JOBS_FLOORS = { previews: 6, queries: 4, thaws: 3, renders: 7 };

/** The accounts' last `[files] native jobs <account>:` lines summed, or null when the front never logged one. */
export const lastJobs = (text) => perAccount(text, "native jobs");

/** Why the jobs fail: a panic, or a floor not reached. */
export function judgeJobs(text, info = null) {
  const r = lastJobs(text);
  if (!r) return ["no [files] native jobs line: the jobs never moved"];
  const why = [];
  if (r.panics !== 0) why.push(`panics=${r.panics}, not 0`);
  floors(r, JOBS_FLOORS, why, info);
  return why;
}

// What the exports must have answered in the conformance run (2573).
const SHARES_FLOORS = { exports: 2, pictures: 1 };

/** The last `[files] native shares:` line's counts, or null when the front never logged one. */
export function lastShares(text) {
  const line = lastLine(text, "[files] native shares:");
  return line ? counts(line) : null;
}

/** Why the shares fail: a panic, or a floor not reached. */
export function judgeShares(text, info = null) {
  const r = lastShares(text);
  if (!r) return ["no [files] native shares line: the shares never moved"];
  const why = [];
  if ((r.panics ?? 0) !== 0) why.push(`panics=${r.panics}, not 0`);
  floors(r, SHARES_FLOORS, why, info);
  return why;
}

/** The last line that counts the shed (`shedlone=`), whatever its prefix, as numbers; null when none. */
export function lastShed(text) {
  const line = lastLine(text, "shedlone=");
  return line ? counts(line) : null;
}

/** Why a run fails on its shed alone: a panic line, a panic counted, or a shed outside a burst (1780a); no floors. */
export function judgeShed(text) {
  const why = [];
  const panics = text
    .split("\n")
    .filter((l) => /\[native\] .* panicked/.test(l));
  if (panics.length)
    why.push(
      `${panics.length} panic(s):\n  ${panics.slice(0, 5).join("\n  ")}`,
    );
  const r = lastShed(text);
  if (r && (r.panics ?? 0) !== 0) why.push(`panics=${r.panics}, not 0`);
  const chains = lastLine(text, "[files] native chains:");
  const chainPanics = chains?.match(/panics=(\d+)/)?.[1];
  if (chainPanics && Number(chainPanics) !== 0)
    why.push(`chains panics=${chainPanics}, not 0`);
  if (r && (r.shedlone ?? 0) > 0)
    why.push(
      `shedlone=${r.shedlone} of shed=${r.shed ?? 0} came with fewer than a burst in flight`,
    );
  return why;
}

// What the engine link must have done in a run of both suites (2262); `polls` is held by POLLS_PER_SECOND instead.
export const ENGINE_FLOORS = { loads: 20, dry: 10 };

// The browser's two-second workspace poll, counted while a project is served: whole web runs read 0.79 to 1.6 a second.
export const POLLS_PER_SECOND = 0.25;

// Conformance has no browser poll; its dry runs (printed 7 to 9) lost the QuickJS prefetch's.
export const CONFORMANCE_FLOORS = { dry: 5, polls: 1 };

/** The polls a run must reach: a rate over the web suite's seconds, one when the suite ran no browser or its length is unknown. */
export function pollFloor(suite, webSeconds) {
  if (suite === "conformance" || !(webSeconds > 0)) return 1;
  return Math.max(1, Math.ceil(webSeconds * POLLS_PER_SECOND));
}

/** Why the engine link fails: a panic on any line, or a counter under its floor, summed over the accounts' last lines. */
export function judgeEngine(text, suite = "both", webSeconds = 0, info = null) {
  const sum = perAccount(text, "native engine") ?? {};
  if (!Object.keys(sum).length)
    return { sum, why: ["no native engine line: the link never moved"] };
  const why = [];
  if (sum.panics !== 0) why.push(`panics=${sum.panics}, not 0`);
  const table =
    suite === "conformance"
      ? { ...ENGINE_FLOORS, ...CONFORMANCE_FLOORS }
      : { ...ENGINE_FLOORS, polls: pollFloor(suite, webSeconds) };
  floors(sum, table, why, info);
  return { sum, why };
}

// What the bed's conformance run must have drawn natively (2482); downloads, takes and prefetches have no floor.
const MEDIA_FLOORS = { waveforms: 6, decodes: 2, prewarms: 1 };

/** Why media fails: a missing line, a panic, or a count under its floor. */
export function judgeMedia(text, info = null) {
  const last = lastLine(text, "[files] native media:");
  if (!last) return ["no [files] native media: line: the piece never answered"];
  const r = counts(last);
  const why = [];
  if ((r.panics ?? 0) !== 0) why.push(`panics=${r.panics}, not 0`);
  floors(r, MEDIA_FLOORS, why, info);
  return why;
}

// The floor of the runtime line (2680): a run whose Logs panel was never read must not read green.
const RUNTIME_FLOORS = { logs: 1 };

// The counts of the runtime line that must stay at zero: none since the account has no runtime of its own.
const RUNTIME_ZEROES = [];

/** Why the runtime line fails: a counter above zero or a floor not reached. */
export function judgeRuntime(text, info = null) {
  const last = lastLine(text, "[files] native runtime:");
  if (!last)
    return ["no [files] native runtime: line: the piece never answered"];
  const r = counts(last);
  const why = [];
  for (const key of RUNTIME_ZEROES)
    if ((r[key] ?? 0) !== 0) why.push(`${key}=${r[key]}, not 0`);
  floors(r, RUNTIME_FLOORS, why, info);
  return why;
}

/** The web suite's length in seconds from its Playwright report, 0 when there is none. */
export function webSeconds(dir) {
  const file = join(dir, "web.json");
  if (!existsSync(file)) return 0;
  try {
    return (JSON.parse(readFileSync(file, "utf8")).stats?.duration ?? 0) / 1000;
  } catch {
    return 0;
  }
}

/** Every judge a suite calls for, as [name, failures, information, what it read]. */
export function judgeAll(text, suite, opts = {}) {
  const { info = false, seconds = 0 } = opts;
  const out = [];
  const run = (name, f, read) => {
    const notes = info ? [] : null;
    const why = f(notes);
    out.push([name, why, notes ?? [], read]);
  };
  run(
    "engine",
    (n) => judgeEngine(text, suite, seconds, n).why,
    `${JSON.stringify(perAccount(text, "native engine"))}, poll floor ${pollFloor(suite, seconds)} (${Math.round(seconds)} s of web)`,
  );
  run(
    "media",
    (n) => judgeMedia(text, n),
    lastLine(text, "[files] native media:"),
  );
  run(
    "runtime",
    (n) => judgeRuntime(text, n),
    lastLine(text, "[files] native runtime:"),
  );
  if (suite !== "web") {
    run("jobs", (n) => judgeJobs(text, n), JSON.stringify(lastJobs(text)));
    run(
      "shares",
      (n) => judgeShares(text, n),
      JSON.stringify(lastShares(text)),
    );
  }
  const shed = lastShed(text);
  const said = shed && {
    shed: shed.shed,
    shedlone: shed.shedlone,
    panics: shed.panics,
  };
  run("shed", () => judgeShed(text), JSON.stringify(said));
  return out;
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  const args = process.argv.slice(2);
  const flags = new Set(args.filter((a) => a.startsWith("--")));
  const [dir, suite = "both"] = args.filter((a) => !a.startsWith("--"));
  const log = dir && join(dir, "logs/front.log");
  if (!log || !["web", "conformance", "both"].includes(suite)) {
    console.error(
      "usage: bedCounters.mjs [--info] <e2e-out> <web|conformance|both>",
    );
    process.exit(2);
  }
  if (!existsSync(log)) {
    console.error(`bed counters FAILED: no front.log in ${dir}/logs`);
    process.exit(1);
  }
  const results = judgeAll(readFileSync(log, "utf8"), suite, {
    info: flags.has("--info"),
    seconds: webSeconds(dir),
  });
  let failed = false;
  for (const [name, why, notes, read] of results) {
    console.log(`bed counters, ${name}: ${read ?? "no line"}`);
    if (notes.length)
      console.log(`  information (filtered run):\n    ${notes.join("\n    ")}`);
    if (why.length) {
      failed = true;
      console.error(`bed counters, ${name} FAILED:\n  ${why.join("\n  ")}`);
    }
  }
  console.log(failed ? "bed counters: FAILED" : "bed counters: green");
  process.exit(failed ? 1 : 0);
}
