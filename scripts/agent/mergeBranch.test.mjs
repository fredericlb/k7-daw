// mergeBranch.test.mjs — merge-branch.sh --dry-run, the proof stamps and merge-queue.sh, run in scratch repos with the script's env overrides.
// Run: node --test scripts/agent/mergeBranch.test.mjs
// What a reader gets wrong: the gate is a stub that counts its runs in a file, so "reused" means the count did not move.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const AGENT = import.meta.dirname;
const git = (cwd, ...args) => {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
};

// A clone with the scripts committed on main, and `feat` checked out in slot-0 with one commit of its own.
function scratch() {
  const dir = mkdtempSync(join(tmpdir(), "k7-merge-"));
  const clone = join(dir, "clone");
  mkdirSync(join(clone, "scripts/agent"), { recursive: true });
  git(clone, "init", "-q", "-b", "main");
  git(clone, "config", "user.email", "t@t");
  git(clone, "config", "user.name", "t");
  for (const f of ["merge-branch.sh", "slot.sh", "proof.sh", "merge-queue.sh"])
    cpSync(join(AGENT, f), join(clone, "scripts/agent", f));
  writeFileSync(join(clone, "base.txt"), "base\n");
  writeFileSync(join(clone, ".gitignore"), ".claude/\n");
  git(clone, "add", "-A");
  git(clone, "commit", "-qm", "base");
  const slot = join(clone, ".claude/worktrees/slot-0");
  git(clone, "worktree", "add", "-q", "-b", "feat", slot, "main");
  writeFileSync(join(slot, "feat.txt"), "feat\n");
  git(slot, "add", "-A");
  git(slot, "commit", "-qm", "k7: feat");
  const count = join(dir, "gate-count");
  const env = {
    ...process.env,
    K7_PROOFS: join(dir, "proofs"),
    K7_GATE: `echo x >> ${count}`,
    K7_REBUILD: "true",
    K7_GENCHECK: "true",
  };
  const runs = () =>
    existsSync(count) ? readFileSync(count, "utf8").split("\n").length - 1 : 0;
  const mb = (...args) =>
    spawnSync("bash", [join(clone, "scripts/agent/merge-branch.sh"), ...args], {
      cwd: dir,
      env,
      encoding: "utf8",
    });
  const onMain = (name) => {
    writeFileSync(join(clone, name), `${name}\n`);
    git(clone, "add", name);
    git(clone, "commit", "-qm", `main ${name}`);
  };
  return { dir, clone, slot, env, runs, mb, onMain };
}

test("a dry run merges main into the branch, gates, prints ready and leaves main alone", () => {
  const s = scratch();
  s.onMain("m1.txt");
  const before = git(s.clone, "rev-parse", "main");
  const r = s.mb("--dry-run", "feat");
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^merge-branch: feat ready$/m);
  assert.equal(git(s.clone, "rev-parse", "main"), before);
  assert.equal(git(s.clone, "rev-list", "--count", "feat..main"), "0");
  assert.equal(s.runs(), 1);
});

test("a second run on the same tree reuses the proof, a tree that moved re-runs the gate", () => {
  const s = scratch();
  assert.equal(s.mb("--dry-run", "feat").status, 0);
  assert.equal(s.runs(), 1);
  const again = s.mb("--dry-run", "feat");
  assert.match(again.stdout, /reused the quick gate's proof/);
  assert.match(again.stdout, /feat ready$/m);
  assert.equal(s.runs(), 1);
  s.onMain("m2.txt");
  const moved = s.mb("--dry-run", "feat");
  assert.doesNotMatch(moved.stdout, /reused/);
  assert.equal(s.runs(), 2);
  // The real merge, after the dry run proved the tree, gates nothing again.
  const real = s.mb("feat");
  assert.equal(real.status, 0, real.stdout + real.stderr);
  assert.match(real.stdout, /merged into main/);
  assert.equal(s.runs(), 2);
});

test("a conflict prints conflicts and a failing gate prints stopped, both exit 1", () => {
  const s = scratch();
  writeFileSync(join(s.clone, "feat.txt"), "other\n");
  git(s.clone, "add", "-A");
  git(s.clone, "commit", "-qm", "main feat.txt");
  const c = s.mb("--dry-run", "feat");
  assert.equal(c.status, 1);
  assert.match(c.stdout, /^merge-branch: feat conflicts$/m);
  const t = scratch();
  t.env.K7_GATE = "false";
  const g = t.mb("--dry-run", "feat");
  assert.equal(g.status, 1);
  assert.match(g.stdout, /^merge-branch: feat stopped$/m);
});

test("the queue merges in order, stops at the first failure and lists the rest as not run", () => {
  const s = scratch();
  const stub = join(s.dir, "stub.sh");
  writeFileSync(
    stub,
    `#!/usr/bin/env bash
echo "merge-branch: $1 starting"
case "$1" in
  bad) echo "merge-branch: the quick gate failed in slot-1"; echo "merge-branch: bad stopped"; exit 1 ;;
  *) echo "merge-branch: $1 merged into main at abc"; exit 0 ;;
esac
`,
  );
  const r = spawnSync(
    "bash",
    [join(s.clone, "scripts/agent/merge-queue.sh"), "one", "bad", "three"],
    {
      cwd: s.dir,
      env: { ...s.env, K7_MERGE_BRANCH: `bash ${stub}`, K7_QUEUE_SLOT: "9" },
      encoding: "utf8",
    },
  );
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^merge-queue: one merged$/m);
  assert.match(
    r.stdout,
    /^merge-queue: bad stopped: the quick gate failed in slot-1$/m,
  );
  assert.match(r.stdout, /^merge-queue: three not run$/m);
  assert.match(r.stdout, /^merge-queue: stopped at bad$/m);
  assert.equal(
    existsSync(join(s.clone, ".claude/state/merge-queue.lock")),
    false,
  );
});
