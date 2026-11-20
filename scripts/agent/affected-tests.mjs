// affected-tests.mjs — the nextest filter for a milestone: a piece's slow tests (twins, file churn) run only when that piece, or one needing it, changed.
// Prints a filterset, or nothing when every test must run. Every test outside the pieces' slow ones always runs (docs/rust-builds.md).
// What a reader gets wrong: it fails closed — a file the table does not claim, or a core file, prints nothing, which means run everything.
import { homedir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { branchBase, branchFiles, run, wideFiles } from "./branch-files.mjs";

const FRONT =
  "apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/";
const FS = "apps/k7-daw-backend/packages/k7-file-service/";

// What each piece needs, from `Switch::parse` (native/mod.rs); `reads` is core, not a piece.
export const NEEDS = {
  docs: [],
  listings: [],
  entries: [],
  revision: [],
  journal: ["entries", "revision"],
  workspaces: ["entries"],
  projects: ["entries", "workspaces", "journal"],
  engine: ["revision", "projects"],
  "engine-jobs": ["engine"],
  shares: ["docs", "engine-jobs"],
};

// The files a piece owns: k7-front's native/ names, k7-file-service's folder and tests/all names.
export const OWNS = {
  docs: [`${FRONT}docs.rs`, `${FRONT}docs_shadow.rs`, `${FRONT}tests_docs`],
  listings: [
    `${FRONT}listings.rs`,
    `${FRONT}tests_listings.rs`,
    `${FRONT}tests_hub.rs`,
    `${FRONT}tests_cost.rs`,
    `${FS}src/listings/`,
    `${FS}tests/all/listings_`,
  ],
  entries: [
    `${FRONT}entries`,
    `${FRONT}twins.rs`,
    `${FRONT}tests_entries`,
    `${FRONT}tests_twins.rs`,
    `${FRONT}tests_moved.rs`,
    `${FS}src/entries/`,
    `${FS}tests/all/entries_`,
  ],
  revision: [
    `${FRONT}revision.rs`,
    `${FRONT}tests_revision.rs`,
    `${FS}src/revision.rs`,
    `${FS}tests/all/revision.rs`,
  ],
  journal: [
    `${FRONT}journal`,
    `${FRONT}undo_shadow.rs`,
    `${FRONT}jtwins.rs`,
    `${FRONT}tests_journal`,
    `${FRONT}tests_jtwins.rs`,
    `${FS}src/journal/`,
    `${FS}tests/all/journal_`,
  ],
  workspaces: [
    `${FRONT}workspaces`,
    `${FRONT}wtwins.rs`,
    `${FRONT}tests_workspaces.rs`,
    `${FRONT}tests_wflight.rs`,
    `${FRONT}tests_wshadow.rs`,
    `${FRONT}tests_wstart.rs`,
    `${FRONT}tests_wtwins.rs`,
    `${FS}src/workspaces/`,
    `${FS}tests/all/workspaces_`,
  ],
  projects: [
    `${FRONT}projects`,
    `${FRONT}ptwins.rs`,
    `${FRONT}tests_projects`,
    `${FRONT}tests_ptwins.rs`,
    `${FS}src/projects/`,
    `${FS}tests/all/projects_`,
  ],
  "engine-jobs": [
    `${FRONT}jobs/`,
    `${FRONT}engine/fake_plugins.rs`,
    `${FS}src/jobs/`,
    `${FS}tests/all/jobs_`,
  ],
  shares: [`${FRONT}shares`, `${FS}src/shares/`, `${FS}tests/all/shares_`],
  engine: [
    `${FRONT}engine/`,
    `${FRONT}engine_switch.rs`,
    `${FRONT}tests_engine_switch.rs`,
    `${FS}src/engine/`,
  ],
};

// A piece's slow tests: its twins in k7-front, its churn tests in k7-file-service (a module prefix, then the churn names).
const front = (mod) => `(package(k7-front) & test(/native::${mod}::/))`;
const churn = (mods) =>
  `(package(k7-file-service) & test(/^(${mods})::/) & test(/hostile|thousand/))`;
export const HEAVY = {
  docs: [],
  listings: [churn("listings_hostile")],
  entries: [front("tests_twins"), churn("entries_hostile")],
  revision: [churn("revision")],
  journal: [front("tests_jtwins"), churn("journal_hostile|journal_records")],
  workspaces: [front("tests_wtwins"), churn("workspaces_hostile")],
  projects: [front("tests_ptwins"), churn("projects_hostile")],
  engine: [],
  "engine-jobs": [],
  shares: [],
};

// A seed range after the first of a twin family: milestones keep one range as the smoke test, the merge gate runs them all (docs/rust-builds.md).
// Ranges end `_seeds_<a>_to_<b>` (a is 1 or 01 for the first); `ptwins` has one test per seed and a rest, so it gets its own rule.
export const SEED_RANGES = [
  "test(/_seeds_(0*[2-9]\\d*|0*1\\d+)_to_\\d+$/)",
  "test(/tests_ptwins::(seeded_sequence_of_twelve_steps_agrees_seed_[2-9]|twenty_eight_more)/)",
];
const noLaterRanges = `not (${SEED_RANGES.join(" | ")})`;

// The k7-front hubs everything reads and the registrations: a change there is no piece's.
const MOD = `${FRONT}mod.rs`;
const registration =
  /^\s*(#\[cfg\(test\)\]|(pub(\([a-z]+\))? )?mod [a-z_0-9]+;)?\s*$/;

const claimed = (file) =>
  Object.keys(OWNS).find((p) => OWNS[p].some((o) => file.startsWith(o)));

// The pieces to run the slow tests of, or `null` for everything. `dirs` are the package folders the workspace names (repo-relative, with a slash).
export function pieces(files, { dirs, registrationOnly = () => false }) {
  if (wideFiles(files).length > 0) return null;
  const changed = new Set();
  for (const f of files) {
    if (!dirs.some((d) => f.startsWith(d))) continue;
    if (f === MOD && registrationOnly(f)) continue;
    const piece = claimed(f);
    if (!piece) return null;
    changed.add(piece);
  }
  // A piece's slow tests also run when a piece it needs changed.
  const kept = new Set(changed);
  for (let grew = true; grew; ) {
    grew = false;
    for (const [p, needs] of Object.entries(NEEDS))
      if (!kept.has(p) && needs.some((n) => kept.has(n))) {
        kept.add(p);
        grew = true;
      }
  }
  return kept;
}

// The milestone's nextest filterset: the pieces' slow tests by change, and only the first seed range of a twin family; empty to run all.
export function filter(files, opts, env = process.env) {
  if (env.K7_TESTS === "all") return "";
  const kept = pieces(files, opts);
  if (!kept) return noLaterRanges;
  const all = Object.values(HEAVY).flat();
  const keep = [...kept].flatMap((p) => HEAVY[p]);
  if (keep.length === all.length) return noLaterRanges;
  const slow = `(not (${all.join(" | ")}))${keep.length ? ` | (${keep.join(" | ")})` : ""}`;
  return `(${slow}) & ${noLaterRanges}`;
}

function main() {
  const root = run("git", ["rev-parse", "--show-toplevel"]).trim();
  const base = branchBase(root);
  const files = branchFiles(root, base);
  // cargo metadata writes target/.rustc_info.json; keep it out of the tree (gate.sh refuses one).
  const meta = JSON.parse(
    run(
      "cargo",
      ["metadata", "--format-version", "1", "--no-deps"],
      resolve(root, "apps/k7-daw-backend"),
      { CARGO_TARGET_DIR: join(homedir(), ".cache", "k7-cargo-metadata") },
    ),
  );
  const dirs = new Set();
  for (const p of meta.packages) {
    dirs.add(`${relative(root, dirname(p.manifest_path))}/`);
    for (const d of p.dependencies.filter((d) => d.path))
      dirs.add(`${relative(root, d.path)}/`);
  }
  // A mod.rs diff whose changed lines all register a module is no core change.
  const registrationOnly = (f) => {
    const diff = run("git", ["diff", "-U0", base, "--", f], root);
    const changed = diff
      .split("\n")
      .filter((l) => /^[+-]/.test(l) && !/^(\+\+\+|---)/.test(l))
      .map((l) => l.slice(1));
    return changed.length > 0 && changed.every((l) => registration.test(l));
  };
  const opts = { dirs: [...dirs], registrationOnly };
  const out = filter(files, opts);
  const kept = pieces(files, opts);
  if (out)
    console.error(
      `affected-tests: slow tests kept for [${[...kept].join(" ")}]`,
    );
  else console.error("affected-tests: every test runs");
  if (out) console.error("affected-tests: later seed ranges skipped");
  console.log(out);
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === resolve(import.meta.filename)
) {
  try {
    main();
  } catch {
    console.log("");
  }
}
