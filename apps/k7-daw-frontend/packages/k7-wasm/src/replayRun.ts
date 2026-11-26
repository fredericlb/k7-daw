// replayRun.ts — the calls the crate's native replays made, re-run through `edit.call`, answers and changes compared.
// A reader would get wrong who decides what is replayed: the native screens do, so the tally equals theirs by construction.

import { editCallEnv, setIdSource } from "./edit.ts";

interface Exec {
  name: string;
  args: unknown[];
  env: unknown;
  ids: string[];
  files: Record<string, string>;
  dirs: string[];
  out: { ret?: unknown; thrown?: string; unsupported?: string };
  changed: Record<string, string | null>;
}

export interface Plan {
  blobs: Record<string, string>;
  execs: Exec[];
}

export const textOf = (plan: Plan, hash: string): string =>
  new TextDecoder().decode(
    Uint8Array.from(atob(plan.blobs[hash] ?? ""), (c) => c.charCodeAt(0)),
  );

/** Where a run keeps the files: paths as the plan spells them, or moved under a temp folder. */
export interface Backend {
  /** Rewrites a text that names a path. */
  map(s: string): string;
  /** Lays the files out; `dirs` exist, empty or not. */
  prepare(files: Record<string, string>, dirs: string[]): void;
  /** Whether the file system folds names to NFD (APFS), which a memory folder does not. */
  foldsNames?: boolean;
  /** Every file now, by path. */
  files(): Record<string, string>;
}

export interface Tally {
  execs: number;
  ret: number;
  thrown: number;
  unsupported: number;
  mismatches: string[];
  /** Calls whose mismatch is the backend's own (a disk that folds names to NFD), named so. */
  departures: string[];
}

/** Where two answers first differ, with what stands around it. */
function firstDiff(a: unknown, b: unknown): string {
  const x = JSON.stringify(a) ?? "";
  const y = JSON.stringify(b) ?? "";
  let i = 0;
  while (i < x.length && x[i] === y[i]) i++;
  return `at ${i}: ${x.slice(Math.max(0, i - 20), i + 40)} / ${y.slice(Math.max(0, i - 20), i + 40)}`;
}

const sorted = (o: Record<string, unknown>) =>
  Object.fromEntries(Object.entries(o).sort(([a], [b]) => (a < b ? -1 : 1)));

const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

/** Re-runs every exec through `edit.call` and compares its answer and its changes with the native ones. */
export function replay(plan: Plan, backend: Backend): Tally {
  const t: Tally = {
    execs: 0,
    ret: 0,
    thrown: 0,
    unsupported: 0,
    mismatches: [],
    departures: [],
  };
  for (const raw of plan.execs) {
    const x = JSON.parse(backend.map(JSON.stringify(raw))) as Exec;
    const before: Record<string, string> = {};
    for (const [p, h] of Object.entries(x.files))
      before[p] = backend.map(textOf(plan, h));
    backend.prepare(before, x.dirs);
    const ids = [...x.ids];
    setIdSource(() => ids.shift() ?? "00000000-0000-4000-8000-000000000001");
    let got: Exec["out"];
    try {
      got = { ret: editCallEnv(x.env, x.name, x.args) };
    } catch (e) {
      const m = (e as Error).message;
      got =
        m.startsWith(`${x.name}: no such edit function`) ||
        m.includes(": no such edit function")
          ? { unsupported: m }
          : { thrown: m };
    }
    setIdSource(undefined);
    const after = backend.files();
    const changed: Record<string, string | null> = {};
    for (const [p, text] of Object.entries(after))
      if (before[p] !== text) changed[p] = text;
    for (const p of Object.keys(before)) if (!(p in after)) changed[p] = null;
    const want: Record<string, string | null> = {};
    for (const [p, h] of Object.entries(x.changed))
      want[p] = h === null ? null : backend.map(textOf(plan, h));
    t.execs++;
    const bad: string[] = [];
    const label = `${x.name} ${JSON.stringify(x.args).slice(0, 120)}`;
    if ("ret" in x.out) {
      t.ret++;
      if (!("ret" in got) || !same(got.ret, x.out.ret))
        bad.push(
          `${label}: returned a different value ${firstDiff(got.ret, x.out.ret)}`,
        );
    } else if ("thrown" in x.out) {
      t.thrown++;
      if (got.thrown !== x.out.thrown)
        bad.push(
          `${label}: threw ${JSON.stringify(got).slice(0, 200)} wanted ${JSON.stringify(x.out.thrown)}`,
        );
    } else {
      t.unsupported++;
      if (!("unsupported" in got))
        bad.push(`${label}: answered, native could not`);
    }
    if (!same(sorted(changed), sorted(want)) && !("unsupported" in x.out))
      bad.push(
        `${label}: changed files differ ${firstDiff(sorted(changed), sorted(want))}`,
      );
    const folded =
      backend.foldsNames && /\P{ASCII}/u.test(JSON.stringify(x.args));
    (folded ? t.departures : t.mismatches).push(...bad);
  }
  return t;
}

/** What the native replays made of the same plan, counted from it. */
export function native(
  plan: Plan,
): Pick<Tally, "execs" | "ret" | "thrown" | "unsupported"> {
  const c = { execs: plan.execs.length, ret: 0, thrown: 0, unsupported: 0 };
  for (const x of plan.execs) {
    if ("ret" in x.out) c.ret++;
    else if ("thrown" in x.out) c.thrown++;
    else c.unsupported++;
  }
  return c;
}
