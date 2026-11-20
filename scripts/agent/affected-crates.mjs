// affected-crates.mjs — the backend workspace's crates a branch's own commits touched, plus every member that depends on them.
// Prints `-p a -p b …` for cargo, nothing when no backend crate is touched, or `--workspace` when it cannot narrow.
// The base is K7_CRATES_BASE (gate.sh --milestone passes the last milestone that passed on this branch), else main's merge-base.
// What a reader gets wrong: a doubt (a workspace file, git or cargo failing) prints `--workspace`; a guess would skip a test.
import { homedir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { branchFiles, run, wideFiles } from "./branch-files.mjs";

const WIDE = "--workspace";
// A crate's own tests, benches and generators: a change there is the crate's alone, no dependent reads them.
const testOnly = (name, rest) =>
  rest.startsWith("tests/") ||
  rest.startsWith("benches/") ||
  (name === "k7-file-service" && rest.startsWith("js/"));

function main() {
  const root = run("git", ["rev-parse", "--show-toplevel"]).trim();
  const ws = resolve(root, "apps/k7-daw-backend");
  const files = branchFiles(root);
  if (wideFiles(files).length > 0) return WIDE;
  // cargo metadata writes target/.rustc_info.json; keep it out of the tree (gate.sh refuses one).
  const meta = JSON.parse(
    run("cargo", ["metadata", "--format-version", "1", "--no-deps"], ws, {
      CARGO_TARGET_DIR: join(homedir(), ".cache", "k7-cargo-metadata"),
    }),
  );
  const members = new Set(meta.workspace_members);
  // Every package the workspace names by path, members and outside crates alike, with its folder relative to the repo.
  const dirs = new Map();
  const deps = new Map();
  for (const p of meta.packages) {
    dirs.set(p.name, `${relative(root, dirname(p.manifest_path))}/`);
    deps.set(
      p.name,
      p.dependencies.filter((d) => d.path).map((d) => d.name),
    );
    for (const d of p.dependencies.filter((d) => d.path)) {
      if (!dirs.has(d.name)) dirs.set(d.name, `${relative(root, d.path)}/`);
    }
  }
  const touched = new Set();
  const alone = new Set();
  for (const f of files) {
    let best = "";
    for (const [name, dir] of dirs)
      if (f.startsWith(dir) && dir.length > (dirs.get(best)?.length ?? 0))
        best = name;
    if (!best) continue;
    if (testOnly(best, f.slice(dirs.get(best).length))) alone.add(best);
    else touched.add(best);
  }
  const hit = new Set(touched);
  for (let grew = true; grew; ) {
    grew = false;
    for (const [name, ds] of deps)
      if (!hit.has(name) && ds.some((d) => hit.has(d))) {
        hit.add(name);
        grew = true;
      }
  }
  const ids = meta.packages
    .filter((p) => members.has(p.id) && (hit.has(p.name) || alone.has(p.name)))
    .map((p) => p.name)
    .sort();
  return ids.map((n) => `-p ${n}`).join(" ");
}

let out;
try {
  out = main();
} catch {
  out = WIDE;
}
console.log(out);
