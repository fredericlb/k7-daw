// affectedTests.test.mjs — pins the milestone's test-selection rules (affected-tests.mjs): narrow on one piece, whole on any doubt.
// What a reader gets wrong: the filter is fail-closed, so most cases here assert an empty string, which means "run everything".
import assert from "node:assert/strict";
import { test } from "node:test";
import { filter, pieces, SEED_RANGES } from "./affected-tests.mjs";

const F =
  "apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/";
const S = "apps/k7-daw-backend/packages/k7-file-service/";
const dirs = [
  "apps/k7-daw-backend/packages/k7-front/",
  "apps/k7-daw-backend/packages/k7-file-service/",
  "apps/k7-daw-wasm/song/",
];
const opts = { dirs };
const LATER = `not (${SEED_RANGES.join(" | ")})`;
const kept = (files, o = opts) => [...(pieces(files, o) ?? [])].sort();

test("a one-piece change keeps that piece's slow tests only and drops the others'", () => {
  const docs = filter([`${F}docs.rs`], opts, {}).replace(
    /^\(|\) & not \(test\(.*$/g,
    "",
  );
  assert.match(
    docs,
    /^\(not \(.*\)\)$/,
    "docs owns no slow test, so nothing is added back",
  );
  assert.match(docs, /tests_ptwins/);
  const listings = filter([`${F}listings.rs`], opts, {}).replace(
    /^\(|\) & not \(test\(.*$/g,
    "",
  );
  const cut = listings.lastIndexOf(")))) | (");
  const [dropped, back] = [listings.slice(0, cut), listings.slice(cut)];
  assert.match(dropped, /tests_wtwins/);
  assert.match(back, /listings_hostile/);
  assert.doesNotMatch(back, /twins|projects_hostile/);
});

test("a core file runs everything", () => {
  for (const core of [
    "shadow.rs",
    "stats.rs",
    "allow.rs",
    "reads.rs",
    "tests.rs",
    "mod.rs",
  ]) {
    assert.equal(filter([`${F}${core}`], opts, {}), LATER, core);
  }
  assert.equal(filter([`${S}src/lib.rs`], opts, {}), LATER);
  assert.equal(filter(["apps/k7-daw-wasm/song/src/lib.rs"], opts, {}), LATER);
});

test("a mod.rs diff that only registers modules is no core change", () => {
  const registrationOnly = () => true;
  assert.deepEqual(
    kept([`${F}mod.rs`, `${F}docs.rs`], { dirs, registrationOnly }),
    ["docs", "shares"],
  );
  assert.equal(pieces([`${F}mod.rs`], { dirs }), null);
});

test("an unknown or new file runs everything, and a workspace file does too", () => {
  assert.equal(filter([`${F}brand_new.rs`], opts, {}), LATER);
  assert.equal(filter([`${F}docs.rs`, `${F}brand_new.rs`], opts, {}), LATER);
  assert.equal(filter([`${S}tests/all/hostile.rs`], opts, {}), LATER);
  assert.equal(
    filter([`${F}docs.rs`, "apps/k7-daw-backend/Cargo.lock"], opts, {}),
    LATER,
  );
  assert.equal(
    filter([`${F}docs.rs`, "scripts/agent/mac-checks.sh"], opts, {}),
    LATER,
  );
});

test("a file outside every crate (a doc, a script) does not widen the run", () => {
  assert.deepEqual(
    kept([`${F}docs.rs`, "docs/agent-playbook.md", "docs/rust-builds.md"]),
    ["docs", "shares"],
  );
});

test("a piece's slow tests also run when a piece it needs changed", () => {
  assert.deepEqual(kept([`${F}entries.rs`]), [
    "engine",
    "engine-jobs",
    "entries",
    "journal",
    "projects",
    "shares",
    "workspaces",
  ]);
  assert.deepEqual(kept([`${F}revision.rs`]), [
    "engine",
    "engine-jobs",
    "journal",
    "projects",
    "revision",
    "shares",
  ]);
  assert.deepEqual(kept([`${S}src/journal/put.rs`]), [
    "engine",
    "engine-jobs",
    "journal",
    "projects",
    "shares",
  ]);
  assert.deepEqual(kept([`${F}engine/flow.rs`]), [
    "engine",
    "engine-jobs",
    "shares",
  ]);
  assert.deepEqual(kept([`${F}engine_switch.rs`]), [
    "engine",
    "engine-jobs",
    "shares",
  ]);
  assert.deepEqual(kept([`${F}listings.rs`]), ["listings"]);
  assert.deepEqual(kept([`${F}ptwins.rs`]), [
    "engine",
    "engine-jobs",
    "projects",
    "shares",
  ]);
});

test("a test-only file belongs to its piece like the source does", () => {
  assert.deepEqual(kept([`${S}tests/all/listings_hostile.rs`]), ["listings"]);
  assert.deepEqual(kept([`${F}tests_wtwins.rs`]), [
    "engine",
    "engine-jobs",
    "projects",
    "shares",
    "workspaces",
  ]);
});

test("when every piece is kept nothing is filtered", () => {
  const files = [
    `${F}entries.rs`,
    `${F}revision.rs`,
    `${F}listings.rs`,
    `${F}docs.rs`,
  ];
  assert.equal(filter(files, opts, {}), LATER);
});

test("K7_TESTS=all forces everything", () => {
  assert.equal(filter([`${F}docs.rs`], opts, { K7_TESTS: "all" }), "");
  assert.notEqual(filter([`${F}docs.rs`], opts, { K7_TESTS: "" }), "");
});

// The test names of the twins on main, as nextest prints them minus the crate path.
const FIRST = [
  "tests_twins::seeded_sequences_of_sixteen_operations_agree_seeds_01_to_16",
  "tests_jtwins::seeded_sequences_of_twelve_steps_agree_seeds_01_to_08",
  "tests_wtwins::seeded_sequences_of_twelve_operations_agree_seeds_01_to_08",
  "tests_ptwins::seeded_sequence_of_twelve_steps_agrees_seed_1",
];
const LATER_NAMES = [
  ...["17_to_32", "33_to_48", "49_to_64"].map(
    (r) =>
      `tests_twins::seeded_sequences_of_sixteen_operations_agree_seeds_${r}`,
  ),
  ...["09_to_16", "17_to_24", "25_to_32"].map(
    (r) => `tests_jtwins::seeded_sequences_of_twelve_steps_agree_seeds_${r}`,
  ),
  ...["09_to_16", "17_to_24", "25_to_32"].map(
    (r) =>
      `tests_wtwins::seeded_sequences_of_twelve_operations_agree_seeds_${r}`,
  ),
  ...[2, 3, 4].map(
    (n) => `tests_ptwins::seeded_sequence_of_twelve_steps_agrees_seed_${n}`,
  ),
  "tests_ptwins::twenty_eight_more_seeded_sequences_agree",
];

test("the milestone keeps the first seed range of each twin family and drops the later ones", () => {
  const rules = SEED_RANGES.map((r) => new RegExp(r.slice(6, -2)));
  const dropped = (n) => rules.some((re) => re.test(n));
  for (const n of FIRST) assert.equal(dropped(n), false, n);
  for (const n of LATER_NAMES) assert.equal(dropped(n), true, n);
  assert.equal(
    dropped("tests_ptwins::the_markers_crossed_on_seed_3_answer_as_before"),
    false,
  );
});

test("the seed rule joins a piece's slow-test filter and K7_TESTS=all lifts it", () => {
  const f = filter([`${F}entries.rs`], opts, {});
  assert.match(f, /^\(.*\) & not \(/);
  assert.equal(filter([`${F}entries.rs`], opts, { K7_TESTS: "all" }), "");
});

test("engine-jobs needs engine and owns its folders", () => {
  assert.ok(kept([`${F}jobs/waiters.rs`]).includes("engine-jobs"));
  assert.deepEqual(kept([`${F}jobs/waiters.rs`]), ["engine-jobs", "shares"]);
  assert.ok(kept([`${F}engine/flow.rs`]).includes("engine-jobs"));
});

test("shares needs docs and engine-jobs and owns its folders", () => {
  assert.deepEqual(kept([`${F}shares/link.rs`]), ["shares"]);
  assert.deepEqual(kept([`${F}shares_switch.rs`]), ["shares"]);
  assert.deepEqual(kept([`${S}tests/all/shares_names.rs`]), ["shares"]);
  assert.ok(kept([`${F}docs.rs`]).includes("shares"));
  assert.ok(kept([`${F}jobs/waiters.rs`]).includes("shares"));
});
