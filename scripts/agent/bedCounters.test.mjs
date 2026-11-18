// bedCounters.test.mjs — the counters fail on a panic and on a floor not reached, never only on a difference.
// Run: node --test scripts/agent/bedCounters.test.mjs

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  judgeAll,
  judgeEngine,
  judgeJobs,
  judgeMedia,
  judgeRuntime,
  judgeShares,
  judgeShed,
  pollFloor,
} from "./bedCounters.mjs";

const kv = (o) =>
  Object.entries(o)
    .map(([k, v]) => `${k}=${v}`)
    .join(" ");

// The front's report line as a bed's front.log prints it, prefixed with its process name.
const shedLine = (over) =>
  `files | [native-shadow] compared logs=7 ${kv({ shed: 0, shedlone: 0, panics: 0, ...over })}`;

test("an answering run is judged on its shed and panics alone, with no floors", () => {
  assert.deepEqual(judgeShed(shedLine({ shed: 192 })), []);
  assert.match(
    judgeShed(shedLine({ shed: 5, shedlone: 2 })).join("\n"),
    /shedlone=2 of shed=5/,
  );
  assert.match(judgeShed(shedLine({ panics: 1 })).join("\n"), /panics=1/);
  assert.match(
    judgeShed("[native] GET /tree panicked").join("\n"),
    /1 panic\(s\)/,
  );
  assert.deepEqual(judgeShed("no report at all"), []);
});

test("an answering run's chains panics fail it", () => {
  const line = (n) =>
    `[files] native chains: lists=4 coldRuns=1 downloads=0 declines=1 panics=${n}\n`;
  assert.deepEqual(judgeShed(line(0) + shedLine({})), []);
  assert.match(judgeShed(line(2) + shedLine({})).join("\n"), /chains panics=2/);
});

const jobsLine = (over, account = "a@b.c") =>
  `[files] native jobs ${account}: ${kv({ renders: 7, freezes: 0, queries: 4, previews: 6, thaws: 3, separations: 0, panics: 0, ...over })}`;

test("the jobs line is judged by its floors and its panics, and a missing line fails", () => {
  assert.deepEqual(judgeJobs(jobsLine({})), []);
  assert.match(
    judgeJobs(jobsLine({ previews: 2 }))[0],
    /previews=2 is under its floor of 6/,
  );
  assert.match(judgeJobs(jobsLine({ panics: 1 }))[0], /panics=1/);
  assert.match(judgeJobs("nothing")[0], /never moved/);
  // The last line of each account is its cumulative one, and the accounts are summed.
  assert.deepEqual(
    judgeJobs(`${jobsLine({ queries: 0 })}\n${jobsLine({})}`),
    [],
  );
  const a = jobsLine({ queries: 2, previews: 3 });
  const b = jobsLine(
    { queries: 2, renders: 0, previews: 3, thaws: 0 },
    "c@d.e",
  );
  assert.deepEqual(judgeJobs(`${a}\n${b}`), []);
});

test("under information a missed floor is collected, a panic still fails", () => {
  const info = [];
  assert.deepEqual(judgeJobs(jobsLine({ previews: 0 }), info), []);
  assert.match(info[0], /previews=0 is under/);
  assert.match(judgeJobs(jobsLine({ panics: 1 }), [])[0], /panics=1/);
});

const engineLine = (account, n) =>
  `files | [files] native engine ${account}@st0.app: loads=${n.loads ?? 0} dry=${n.dry ?? 0} resends=0 redials=0 copies=0 polls=${n.polls ?? 0} panics=${n.panics ?? 0}`;

test("the engine link's counters are summed over the accounts' last lines and held to their floors", () => {
  const ok = [
    engineLine("a", { loads: 3, dry: 2, polls: 100 }),
    engineLine("a", { loads: 12, dry: 6, polls: 300 }),
    engineLine("b", { loads: 10, dry: 5, polls: 250 }),
  ].join("\n");
  const got = judgeEngine(ok, "both", 600);
  assert.deepEqual(got.why, []);
  assert.equal(got.sum.loads, 22);
  assert.equal(got.sum.polls, 550);
  assert.match(
    judgeEngine(engineLine("a", { loads: 19, dry: 10, polls: 500 })).why.join(
      "\n",
    ),
    /loads=19 is under its floor of 20/,
  );
  assert.match(
    judgeEngine(
      engineLine("a", { loads: 20, dry: 10, polls: 500, panics: 1 }),
    ).why.join("\n"),
    /panics=1/,
  );
  assert.match(judgeEngine("nothing").why.join("\n"), /never moved/);
});

test("the poll floor is a rate over the web suite's length: a fast or loaded run passes, a link that never polls fails", () => {
  // The Mac's whole run at load 30 (398 s, 456 polls) and Linux's (631 s, 499 polls): both under the old fixed 500.
  const mac = engineLine("a", { loads: 206, dry: 115, polls: 456 });
  assert.deepEqual(judgeEngine(mac, "both", 398).why, []);
  const linux = engineLine("a", { loads: 206, dry: 112, polls: 499 });
  assert.deepEqual(judgeEngine(linux, "web", 631).why, []);
  assert.equal(pollFloor("both", 631), 158);
  // A link the poll no longer reaches: a handful of polls over a whole web run.
  const silent = engineLine("a", { loads: 206, dry: 112, polls: 12 });
  assert.match(
    judgeEngine(silent, "both", 600).why.join("\n"),
    /polls=12 is under its floor of 150/,
  );
  // No web report: one poll, the least that proves the route reaches the link.
  assert.equal(pollFloor("both", 0), 1);
});

test("a conformance-only run needs one poll, whatever its length", () => {
  const line = engineLine("a", { loads: 20, dry: 10, polls: 62 });
  assert.deepEqual(judgeEngine(line, "conformance", 0).why, []);
  assert.equal(pollFloor("conformance", 900), 1);
  const few = engineLine("a", { loads: 20, dry: 4, polls: 62 });
  assert.match(
    judgeEngine(few, "conformance").why.join("\n"),
    /dry=4 is under its floor of 5/,
  );
  const none = engineLine("a", { loads: 20, dry: 10, polls: 0 });
  assert.match(
    judgeEngine(none, "conformance").why.join("\n"),
    /polls=0 is under its floor of 1/,
  );
});

test("a panic in an account's early line fails the judge although its last line and another account's read zero", () => {
  const text = [
    engineLine("a", { loads: 5, panics: 1 }),
    engineLine("b", { loads: 5 }),
    engineLine("a", { loads: 20, dry: 10, polls: 500 }),
    engineLine("b", { loads: 20, dry: 10, polls: 500 }),
  ].join("\n");
  assert.match(judgeEngine(text).why.join("\n"), /panics=1/);
});

const sharesLine = (over) =>
  `[files] native shares: ${kv({ exports: 2, pictures: 1, declines: 0, panics: 0, ...over })}`;

test("shares: a panic or a floor not reached fails", () => {
  assert.deepEqual(judgeShares(sharesLine({})), []);
  assert.match(
    judgeShares(sharesLine({ exports: 1 }))[0],
    /exports=1 is under its floor of 2/,
  );
  assert.match(judgeShares(sharesLine({ panics: 1 }))[0], /panics=1/);
  assert.match(judgeShares("nothing")[0], /never moved/);
  assert.deepEqual(
    judgeShares(`${sharesLine({ exports: 0 })}\n${sharesLine({})}`),
    [],
  );
});

const mediaLine = (over) =>
  `[files] native media: ${kv({ waveforms: 6, decodes: 2, prewarms: 1, downloads: 0, takes: 0, prefetches: 0, declines: 0, panics: 0, ...over })}`;

test("media fails on a missing line, a panic and each floor, and holds on its floors", () => {
  assert.deepEqual(judgeMedia(mediaLine({})), []);
  assert.match(judgeMedia("nothing")[0], /never answered/);
  assert.match(judgeMedia(mediaLine({ panics: 1 }))[0], /panics=1/);
  for (const key of ["waveforms", "decodes", "prewarms"])
    assert.match(
      judgeMedia(mediaLine({ [key]: 0 }))[0],
      new RegExp(`${key}=0 is under`),
    );
  assert.deepEqual(
    judgeMedia(`${mediaLine({ waveforms: 0 })}\n${mediaLine({})}`),
    [],
  );
});

const runtimeLine = (over) =>
  `[files] native runtime: ${kv({ background: 0, logs: 5, ...over })}`;

test("the runtime line fails on a floor not reached", () => {
  assert.deepEqual(judgeRuntime(runtimeLine({})), []);
  assert.match(judgeRuntime("nothing")[0], /never answered/);
  assert.match(
    judgeRuntime(runtimeLine({ logs: 0 }))[0],
    /logs=0 is under its floor of 1/,
  );
  assert.deepEqual(
    judgeRuntime(`${runtimeLine({ logs: 0 })}\n${runtimeLine({})}`),
    [],
  );
});

const wholeLog = [
  engineLine("a", { loads: 30, dry: 12, polls: 400 }),
  jobsLine({}),
  sharesLine({}),
  mediaLine({}),
  runtimeLine({}),
  shedLine({}),
].join("\n");

test("a web run skips the jobs and the shares, which only conformance asks for", () => {
  const names = (suite) =>
    judgeAll(wholeLog, suite, { seconds: 600 }).map(([n]) => n);
  assert.deepEqual(names("web"), ["engine", "media", "runtime", "shed"]);
  assert.deepEqual(names("both"), [
    "engine",
    "media",
    "runtime",
    "jobs",
    "shares",
    "shed",
  ]);
  for (const [, why] of judgeAll(wholeLog, "both", { seconds: 600 }))
    assert.deepEqual(why, []);
});

test("the command reads an e2e-out folder: green, failed on a floor, informed on a filtered run", () => {
  const dir = mkdtempSync(join(tmpdir(), "bedcounters-"));
  mkdirSync(join(dir, "logs"));
  const run = (log, seconds, ...flags) => {
    writeFileSync(join(dir, "logs/front.log"), log);
    writeFileSync(
      join(dir, "web.json"),
      JSON.stringify({ stats: { duration: seconds * 1000 } }),
    );
    return spawnSync(
      process.execPath,
      [
        new URL("./bedCounters.mjs", import.meta.url).pathname,
        ...flags,
        dir,
        "both",
      ],
      { encoding: "utf8" },
    );
  };
  const green = run(wholeLog, 600);
  assert.equal(green.status, 0, green.stderr);
  assert.match(green.stdout, /poll floor 150 \(600 s of web\)/);
  const slow = run(wholeLog, 2000);
  assert.equal(slow.status, 1);
  assert.match(slow.stderr, /polls=400 is under its floor of 500/);
  const filtered = run(wholeLog, 2000, "--info");
  assert.equal(filtered.status, 0);
  assert.match(filtered.stdout, /information/);
  const panicked = run(
    `${wholeLog}\n${jobsLine({ panics: 1 })}`,
    600,
    "--info",
  );
  assert.equal(panicked.status, 1);
  assert.equal(
    spawnSync(
      process.execPath,
      [new URL("./bedCounters.mjs", import.meta.url).pathname],
      {
        encoding: "utf8",
      },
    ).status,
    2,
  );
});
