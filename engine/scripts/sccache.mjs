// sccache.mjs — finds sccache and gives the environment that sends a build's rustc (and, for a CI tree, its
// C++ compiler) through it. Never required: a machine without sccache builds uncached and says so once.
// The trap: a server keeps the folder and size it was started with, so each use has its own port (docs/rust-builds.md).
// `node engine/scripts/sccache.mjs <use>` prints the variables as KEY=VALUE lines, for the .cmd steps and the sh loops that read lines.
// `--export` prints `export KEY='VALUE'` instead, for `eval`: a bare KEY=VALUE through eval sets shell variables cargo never sees.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const windows = process.platform === "win32";

/** The shell agents' folder for OneDev's lasting trees and caches; outside Windows not ~/k7-ci, which the macOS checks prune. */
export const ciRoot = () =>
  windows
    ? join(process.env.LOCALAPPDATA ?? homedir(), "k7-ci")
    : join(homedir(), "k7-trees");

// dir under ciRoot(), or none for sccache's own default; the sizes add up to the 30 GB docs/rust-builds.md gives it.
const USES = {
  dev: { size: "10G" },
  // An agent's builds: dev's cache, a fixed share of the cores, and no debugger's debug info (docs/rust-builds.md).
  agent: {
    size: "10G",
    extra: {
      CARGO_BUILD_JOBS: "4",
      NEXTEST_TEST_THREADS: "8",
      CARGO_PROFILE_DEV_DEBUG: "line-tables-only",
    },
  },
  tree: { dir: "sccache", size: "15G", port: "4227" },
  checks: { dir: "sccache-rust", size: "5G", port: "4228" },
};

/** sccache's path: the PATH, then WinGet's package folder, then Homebrew's and cargo's bins. */
function findSccache() {
  const where = spawnSync(windows ? "where" : "which", ["sccache"], {
    encoding: "utf8",
  });
  const onPath =
    where.status === 0 ? where.stdout.split(/\r?\n/)[0].trim() : "";
  if (onPath) return onPath;
  const winget = join(
    process.env.LOCALAPPDATA ?? "",
    "Microsoft",
    "WinGet",
    "Packages",
  );
  const pkg =
    windows && existsSync(winget)
      ? readdirSync(winget).find((d) => d.startsWith("Mozilla.sccache"))
      : undefined;
  const candidates = [
    ...(pkg
      ? readdirSync(join(winget, pkg)).map((d) =>
          join(winget, pkg, d, "sccache.exe"),
        )
      : []),
    ...(windows ? [] : ["/opt/homebrew/bin/sccache", "/usr/local/bin/sccache"]),
    join(homedir(), ".cargo", "bin", windows ? "sccache.exe" : "sccache"),
  ];
  return candidates.find((p) => existsSync(p)) ?? null;
}

/** The variables that put <use>'s builds through sccache, and its `extra` always; an RUSTC_WRAPPER already set wins. */
export function sccacheEnv(use, { log = console.log, env = process.env } = {}) {
  const spec = USES[use];
  if (!spec) throw new Error(`sccache.mjs: no use "${use}"`);
  const extra = spec.extra ?? {};
  if (env.RUSTC_WRAPPER !== undefined) return { ...extra };
  const exe = findSccache();
  if (!exe) {
    log("sccache: not on this machine, compiling uncached");
    return { ...extra };
  }
  const port = env.SCCACHE_SERVER_PORT ?? userPort(spec.port);
  return {
    ...extra,
    RUSTC_WRAPPER: exe,
    SCCACHE_CACHE_SIZE: spec.size,
    ...(spec.dir && { SCCACHE_DIR: join(ciRoot(), spec.dir) }),
    ...(port && { SCCACHE_SERVER_PORT: port }),
  };
}

/** A use's port moved by the account's uid outside Windows: a server runs as the user who started it, and writes as that user. */
export function userPort(base, uid = windows ? undefined : process.getuid?.()) {
  if (uid === undefined) return base;
  return String(Number(base ?? "4226") + 100 * (uid % 100));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const vars = sccacheEnv(process.argv[2] ?? "", { log: console.error });
  const exported = process.argv.includes("--export");
  for (const [k, v] of Object.entries(vars))
    console.log(
      exported ? `export ${k}='${v.replaceAll("'", "'\\''")}'` : `${k}=${v}`,
    );
}
