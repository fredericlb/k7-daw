// ciTree.mjs — the shell agents' OneDev jobs build in a tree that outlives the run: each run checks its
// commit out there, so git rewrites only the files the push changed and the build folders stay warm.
// One run per tree at a time, by a lock file whose holder may have been killed by a cancel. The step
// itself is a .cmd (Windows) or a .sh (macOS) under engine/scripts/ci/, run in the tree; its dist/release
// comes back to the workspace.
// The agent's checkout token reaches git alone, never the build (docs/release.md, "On OneDev").

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  closeSync,
  cpSync,
  existsSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { ciRoot as ciRootOf, sccacheEnv } from "./sccache.mjs";

const { values: opt } = parseArgs({
  strict: true,
  options: {
    tree: { type: "string" },
    repo: { type: "string" },
    commit: { type: "string" },
    "fetch-dir": { type: "string" },
    run: { type: "string" },
  },
});
for (const k of ["tree", "repo", "commit", "fetch-dir", "run"])
  if (!opt[k]) throw new Error(`ciTree: --${k} is required`);

const workspace = process.cwd();
const windows = process.platform === "win32";
const ciRoot = ciRootOf();
const tree = join(ciRoot, `tree-${opt.tree}`);
const lockFile = `${tree}.lock`;
const LOCK_STALE_MS = 4 * 3600_000;

// The token header the job put in GIT_CONFIG_*: kept for git, taken out of the build's environment.
const gitEnv = { ...process.env };
const buildEnv = Object.fromEntries(
  Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_CONFIG_")),
);

function sh(cmd, args, { cwd = tree, env = gitEnv, ok = false } = {}) {
  console.log(`▸ ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, {
    cwd,
    env,
    stdio: ok ? "pipe" : "inherit",
    encoding: "utf8",
    shell: cmd === "npm",
  });
  if (ok) return r.status === 0 ? r.stdout.trim() : null;
  if (r.status !== 0)
    throw new Error(`${cmd} ${args[0]} failed (exit ${r.status})`);
  return "";
}

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
};

async function lock() {
  for (;;) {
    try {
      const fd = openSync(lockFile, "wx");
      writeFileSync(fd, JSON.stringify({ pid: process.pid, at: Date.now() }));
      closeSync(fd);
      return;
    } catch (e) {
      if (e.code !== "EEXIST") throw e;
    }
    let held = null;
    try {
      held = JSON.parse(readFileSync(lockFile, "utf8"));
    } catch {}
    if (!held || !alive(held.pid) || Date.now() - held.at > LOCK_STALE_MS) {
      console.log(`ciTree: taking over ${lockFile}, its holder is gone`);
      rmSync(lockFile, { force: true });
      continue;
    }
    console.log(`ciTree: ${opt.tree} is busy (pid ${held.pid}), waiting`);
    await new Promise((r) => setTimeout(r, 15_000));
  }
}

const unlock = () => rmSync(lockFile, { force: true });

function prepare() {
  const fresh = !existsSync(join(tree, ".git"));
  if (fresh) {
    mkdirSync(tree, { recursive: true });
    sh("git", ["init", "-q"]);
    sh("git", ["remote", "add", "origin", opt.repo]);
  } else sh("git", ["remote", "set-url", "origin", opt.repo]);
  sh("git", ["config", "core.longpaths", "true"]);
  sh("git", [
    "fetch",
    "-q",
    "--force",
    "--tags",
    "origin",
    "+refs/heads/*:refs/remotes/origin/*",
  ]);
  // A commit, a tag, or a branch name read as origin's: a by-hand run names a ref (docs/release.md).
  const known = (rev) =>
    sh("git", ["cat-file", "-e", `${rev}^{commit}`], { ok: true }) !== null;
  let rev = [opt.commit, `origin/${opt.commit}`].find(known);
  if (!rev) {
    sh("git", ["fetch", "-q", "origin", opt.commit]);
    rev = "FETCH_HEAD";
  }
  sh("git", ["checkout", "-q", "-f", "--detach", rev]);
  // Untracked files go, ignored ones stay: build/, node_modules/ and Gradle's outputs are the point.
  sh("git", ["clean", "-ffdq"]);
  rmSync(join(tree, "dist", "release"), { recursive: true, force: true });
  // The dependency builds there were made for another tree or generator, so they start over once per tree.
  const wiped = join(tree, ".git", "k7-ci-deps-wiped");
  if (!existsSync(wiped) && wipeDependencyBuilds()) writeFileSync(wiped, "");
  const lockHash = createHash("sha256")
    .update(readFileSync(join(tree, "package-lock.json")))
    .digest("hex");
  const hashFile = join(tree, ".git", "k7-ci-npm-hash");
  const installed =
    existsSync(join(tree, "node_modules")) &&
    existsSync(hashFile) &&
    readFileSync(hashFile, "utf8") === lockHash;
  if (!installed) {
    sh("npm", ["ci", "--no-audit", "--no-fund"], { env: buildEnv });
    writeFileSync(hashFile, lockHash);
  } else console.log("ciTree: node_modules matches the lockfile, no install");
}

/** Whether every dependency build went; a file held open leaves the wipe for the next run. */
function wipeDependencyBuilds() {
  const dir = opt["fetch-dir"];
  if (!existsSync(dir)) return true;
  let done = true;
  for (const project of readdirSync(dir))
    for (const entry of existsSync(join(dir, project))
      ? readdirSync(join(dir, project))
      : [])
      if (entry.endsWith("-build") || entry.endsWith("-subbuild"))
        try {
          rmSync(join(dir, project, entry), { recursive: true, force: true });
        } catch (e) {
          console.log(
            `ciTree: could not wipe ${entry} (${e.code}), trying again next run`,
          );
          done = false;
        }
  return done;
}

function stamp() {
  const tag = sh(
    "git",
    ["describe", "--tags", "--exact-match", "--match", "v*", "HEAD"],
    { ok: true },
  );
  // A fixed length: each agent's tree names the same commit, and git's own default follows the clone's size.
  return (
    tag ||
    `snapshot-${sh("git", ["rev-parse", "--short=9", "HEAD"], { ok: true })}`
  );
}

mkdirSync(ciRoot, { recursive: true });
await lock();
process.on("exit", unlock);
for (const sig of ["SIGINT", "SIGTERM", "SIGBREAK"])
  process.on(sig, () => process.exit(130));
prepare();
const cache = sccacheEnv("tree", { env: buildEnv });
const sccache = cache.RUSTC_WRAPPER;
const env = {
  ...buildEnv,
  STUDIO_FETCHCONTENT_DIR: opt["fetch-dir"],
  STUDIO_RELEASE_STAMP: stamp(),
  K7_CI_WORKSPACE: workspace,
  ...cache,
  ...(sccache && { STUDIO_COMPILER_LAUNCHER: sccache }),
};
console.log(
  `ciTree: ${opt.tree} at ${opt.commit}, stamp ${env.STUDIO_RELEASE_STAMP}`,
);
if (sccache) sh(sccache, ["--start-server"], { env, ok: true });
if (windows) sh("cmd.exe", ["/d", "/c", resolve(tree, opt.run)], { env });
else sh("bash", [resolve(tree, opt.run)], { env });
// Left running for its idle timeout: the engine and the APK jobs share this server, and a stop would cut the other.
if (sccache) sh(sccache, ["--show-stats"], { env });
const out = join(tree, "dist", "release");
if (existsSync(out))
  cpSync(out, join(workspace, "dist", "release"), { recursive: true });
