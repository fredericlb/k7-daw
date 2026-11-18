// gateLock.test.mjs — gate-lib.sh's lock and rerun-failed.sh's quiet-machine wait, run through bash with env overrides.
// Run: node --test scripts/agent/gateLock.test.mjs
// What a reader gets wrong: the load is faked with K7_LOAD, so no real machine state is read.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const AGENT = import.meta.dirname;
const bash = (script, env = {}) =>
  spawnSync("bash", ["-c", script], {
    encoding: "utf8",
    env: { ...process.env, K7_GATE_LOCK_POLL: "0.1", ...env },
  });
const lockDir = () => join(mkdtempSync(join(tmpdir(), "k7-lock-")), "l.lock");

test("a free lock is taken with pid and branch, then released", () => {
  const dir = lockDir();
  const r = bash(
    `. ${AGENT}/gate-lib.sh; gate_lock_take feat; cat "$K7_GATE_LOCK_DIR/branch"; gate_lock_release; [ ! -d "$K7_GATE_LOCK_DIR" ] && echo gone`,
    { K7_GATE_LOCK_DIR: dir },
  );
  assert.equal(r.stdout.trim(), "feat\ngone");
});

test("a stale lock (dead pid) is cleared", () => {
  const dir = lockDir();
  mkdirSync(dir);
  writeFileSync(join(dir, "pid"), "999999\n");
  const r = bash(`. ${AGENT}/gate-lib.sh; gate_lock_take b; echo ok`, {
    K7_GATE_LOCK_DIR: dir,
  });
  assert.equal(r.stdout.trim(), "ok");
});

test("a live holder makes the second gate wait and name it", () => {
  const dir = lockDir();
  const r = bash(
    `. ${AGENT}/gate-lib.sh
     sleep 30 & holder=$!
     mkdir "$K7_GATE_LOCK_DIR"; echo $holder > "$K7_GATE_LOCK_DIR/pid"; echo other > "$K7_GATE_LOCK_DIR/branch"
     ( sleep 0.5; kill $holder; ) &
     gate_lock_take mine; echo taken`,
    { K7_GATE_LOCK_DIR: dir },
  );
  assert.match(r.stdout, /waiting for the gate lock held by pid \d+ \(other\)/);
  assert.match(r.stdout, /taken/);
});

test("K7_GATE_NO_LOCK bypasses the lock", () => {
  const dir = lockDir();
  mkdirSync(dir);
  writeFileSync(join(dir, "pid"), `${process.pid}\n`);
  const r = bash(`. ${AGENT}/gate-lib.sh; gate_lock_take b; echo ok`, {
    K7_GATE_LOCK_DIR: dir,
    K7_GATE_NO_LOCK: "1",
  });
  assert.equal(r.stdout.trim(), "ok");
});

test("the rerun waits while the load is high, then gives up and says so", () => {
  const r = bash(`. ${AGENT}/rerun-failed.sh; rerun_wait_quiet`, {
    K7_LOAD: "9999",
    K7_QUIET_POLL: "1",
    K7_QUIET_MAX: "2",
  });
  assert.match(r.stdout, /waiting for a quiet machine/);
  assert.match(r.stdout, /gave up waiting for a quiet machine/);
});

test("a quiet machine does not wait", () => {
  const r = bash(`. ${AGENT}/rerun-failed.sh; rerun_wait_quiet; echo done`, {
    K7_LOAD: "0",
  });
  assert.equal(r.stdout.trim(), "done");
});

test("gate.sh --base sets the merge-base the changed files are read from", () => {
  const dir = mkdtempSync(join(tmpdir(), "k7-base-"));
  const g = (...a) => spawnSync("git", a, { cwd: dir, encoding: "utf8" });
  g("init", "-q", "-b", "main");
  for (const [b, f] of [
    ["main", "a"],
    ["parent", "p"],
    ["child", "c"],
  ]) {
    if (b !== "main") g("checkout", "-q", "-b", b);
    writeFileSync(join(dir, f), f);
    g("add", ".");
    g("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", f);
  }
  const files = (base) =>
    g("diff", "--name-only", g("merge-base", "HEAD", base).stdout.trim())
      .stdout.trim()
      .split("\n");
  assert.deepEqual(files("main"), ["c", "p"]);
  assert.deepEqual(files("parent"), ["c"]);
  assert.ok(existsSync(join(AGENT, "gate.sh")));
});

test("the timeout scale is 4 above the CPU count, 1 below, and an explicit value stays", () => {
  const scale = (env) =>
    bash(
      `. ${AGENT}/gate-lib.sh; gate_timeout_scale; echo $K7_TEST_TIMEOUT_SCALE`,
      {
        K7_TEST_TIMEOUT_SCALE: "",
        ...env,
      },
    ).stdout.trim();
  assert.equal(scale({ K7_LOAD: "100000" }), "4");
  assert.equal(scale({ K7_LOAD: "0" }), "1");
  assert.equal(scale({ K7_TEST_TIMEOUT_SCALE: "7", K7_LOAD: "100000" }), "7");
});
