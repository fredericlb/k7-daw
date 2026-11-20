// makeEditVectors.mts — writes rust/tests/vectors/edit.json from the TypeScript editor and printer (docs/wasm-step3a-brief.md).
// Run: npx tsx apps/k7-daw-frontend/packages/k7x/scripts/makeEditVectors.mts. What reads wrong: the corpus ops are generated twice, here and in rust/tests/corpus.rs.

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative } from "node:path";
import { type K7Node, toJS } from "../src/ast.ts";
import { K7Document } from "../src/edit.ts";
import { stringify } from "../src/print.ts";
import { applySugared } from "../src/sugared.ts";

const PKG = join(import.meta.dirname, "..");
const ROOT = join(PKG, "..", "..", "..", "..");
const OUT = join(
  PKG,
  "..",
  "..",
  "..",
  "k7-daw-wasm",
  "k7x",
  "tests",
  "vectors",
);

type Path = (string | number)[];
type Op = { op: string; path?: Path; [k: string]: unknown };
type Result = { t?: string; e?: string; g?: unknown };

// ---- the sugar double: a short form is a key whose value is the target's value ----
interface SugarSpec {
  parts: { key: string; target: string }[];
}
function sugarOf(spec: SugarSpec) {
  const has = (raw: Record<string, unknown>, k: string) =>
    Object.hasOwn(raw, k) && raw[k] !== undefined;
  return {
    parts: spec.parts.map((p) => ({
      ...p,
      present: (raw: Record<string, unknown>) => has(raw, p.key),
    })),
    expand: (raw: Record<string, unknown>) => {
      const out: Record<string, unknown> = { ...raw };
      for (const p of spec.parts)
        if (has(raw, p.key) && p.target !== p.key) {
          delete out[p.key];
          out[p.target] = raw[p.key];
        }
      return out;
    },
  };
}

// ---- the values: JSON, with -0, NaN and the infinities tagged ----
const enc = (v: unknown): unknown =>
  JSON.parse(
    JSON.stringify(v, (_k, x) =>
      typeof x === "number" && (!Number.isFinite(x) || Object.is(x, -0))
        ? { $num: Object.is(x, -0) ? "-0" : String(x) }
        : x,
    ),
  );

function fnv(s: string): string {
  const b = Buffer.from(s, "utf8");
  let h1 = 0x811c9dc5;
  let h2 = (0x01000193 ^ 0x9e3779b9) >>> 0;
  for (const c of b) {
    h1 = Math.imul(h1 ^ c, 0x01000193) >>> 0;
    h2 = Math.imul(h2 ^ c, 0x85ebca6b) >>> 0;
  }
  return h1.toString(16).padStart(8, "0") + h2.toString(16).padStart(8, "0");
}

// ---- running one op ----
function exec(doc: K7Document, op: Op): unknown {
  const path = op.path ?? [];
  const o = op.inline ? { inline: true } : undefined;
  switch (op.op) {
    case "setIn":
      return void doc.setIn(path, op.value, o);
    case "setSourceIn":
      return void doc.setSourceIn(path, op.source as string);
    case "addIn":
      return void doc.addIn(path, op.value, o);
    case "replaceSource":
      return void doc.replaceSource(path, op.source as string);
    case "insertIn":
      return void doc.insertIn(path, op.index as number, op.value, o);
    case "insertSourceIn":
      return void doc.insertSourceIn(
        path,
        op.index as number,
        op.source as string,
      );
    case "moveIn":
      return void doc.moveIn(path, op.from as number, op.to as number);
    case "renameKey":
      return void doc.renameKey(path, op.key as string);
    case "applyIn":
      return void doc.applyIn(path, op.value);
    case "deleteIn":
      return void doc.deleteIn(path, op.prune ? { prune: true } : undefined);
    case "setHeader":
      return void doc.setHeader(op.lines as string[]);
    case "applySugared":
      return void applySugared(
        doc,
        op.after,
        op.sugar ? sugarOf(op.sugar as SugarSpec) : null,
      );
    case "getIn":
      return { g: doc.getIn(path) ?? null, has: doc.getIn(path) !== undefined };
    case "getSource":
      return { g: doc.getSource(path) ?? null };
    case "positionOf":
      return { g: doc.positionOf(path) ?? null };
    default:
      throw new Error(`unknown op ${op.op}`);
  }
}

/** One op on `doc`: the result with the text after it, or the thrown message. */
function step(doc: K7Document, op: Op): Result {
  try {
    const got = exec(doc, op) as { g?: unknown; has?: boolean } | undefined;
    return got === undefined
      ? { t: doc.toString() }
      : { t: doc.toString(), g: enc(got) };
  } catch (e) {
    return { e: (e as Error).message };
  }
}

const independent = (text: string, ops: Op[]): Result[] =>
  ops.map((op) => step(new K7Document(text), op));

function chained(
  text: string,
  ops: Op[],
): { results: Result[]; changes: unknown[] } {
  const doc = new K7Document(text);
  const results = ops.map((op) => step(doc, op));
  return {
    results,
    changes: doc.changes.map((c) => [c.start, c.end, c.insert]),
  };
}

// ---- the ops a text gets, from its tree: generated here and again in rust/tests/corpus.rs ----
interface Shape {
  scalars: { path: Path; value: unknown }[];
  entries: Path[];
  maps: Path[];
  lists: { path: Path; len: number }[];
}

function shape(root: K7Node): Shape {
  const s: Shape = { scalars: [], entries: [], maps: [], lists: [] };
  const go = (n: K7Node, at: Path): void => {
    if (n.kind === "scalar") s.scalars.push({ path: at, value: n.value });
    else if (n.kind === "list") {
      s.lists.push({ path: at, len: n.items.length });
      for (const [i, x] of n.items.entries()) go(x, [...at, i]);
    } else {
      s.maps.push(at);
      for (const e of n.entries) {
        s.entries.push([...at, e.key]);
        go(e.value, [...at, e.key]);
      }
    }
  };
  go(root, []);
  return s;
}

const changed = (v: unknown): unknown =>
  typeof v === "number"
    ? v + 1
    : typeof v === "string"
      ? `${v}x`
      : typeof v === "boolean"
        ? !v
        : 1;

function mutate(v: unknown, c: { n: number }): unknown {
  if (Array.isArray(v)) {
    const out = v.map((x) => mutate(x, c));
    c.n += 1;
    if (out.length >= 3 && c.n % 3 === 0) out.splice(1, 1);
    else if (c.n % 4 === 0) out.push("m");
    return out;
  }
  if (v !== null && typeof v === "object") {
    const out: [string, unknown][] = [];
    for (const [k, x] of Object.entries(v)) {
      c.n += 1;
      if (c.n % 7 === 0) continue;
      out.push([k, mutate(x, c)]);
    }
    c.n += 1;
    if (c.n % 11 === 0) out.push(["k7xAdded", c.n]);
    return Object.fromEntries(out);
  }
  c.n += 1;
  return c.n % 5 === 0 ? changed(v) : v;
}

const LONG = "L".repeat(120);

function opsFor(root: K7Node, data: unknown, take: number): Op[] {
  const s = shape(root);
  const ops: Op[] = [];
  for (const { path, value } of s.scalars.slice(0, take))
    ops.push({ op: "setIn", path, value: changed(value) });
  for (const path of s.entries.slice(0, take))
    ops.push({ op: "deleteIn", path });
  for (const path of s.entries.slice(0, take / 2))
    ops.push({ op: "deleteIn", path, prune: true });
  for (const path of s.maps.slice(0, take))
    ops.push({
      op: "setIn",
      path: [...path, "k7xProbe"],
      value: { a: [1, "two"] },
    });
  for (const { path, len } of s.lists.slice(0, take / 2)) {
    ops.push({ op: "insertIn", path, index: 0, value: "ins" });
    ops.push({ op: "addIn", path, value: { x: 1 } });
    ops.push({
      op: "insertIn",
      path,
      index: Math.floor(len / 2),
      value: [1, 2],
    });
    ops.push({ op: "insertSourceIn", path, index: len, source: "7 /* c */" });
  }
  for (const { path, len } of s.lists
    .filter((l) => l.len >= 2)
    .slice(0, take / 2))
    ops.push({ op: "moveIn", path, from: 0, to: len - 1 });
  for (const path of s.entries.slice(0, take / 2))
    ops.push({
      op: "renameKey",
      path,
      key: `${String(path[path.length - 1])}_r`,
    });
  for (const { path } of s.scalars.slice(0, take / 2))
    ops.push({ op: "setSourceIn", path, source: "[1, 2]" });
  for (const { path } of s.scalars.slice(0, take / 2))
    ops.push({ op: "setIn", path, value: LONG, inline: true });
  ops.push({ op: "applyIn", path: [], value: mutate(data, { n: 0 }) });
  ops.push({ op: "setHeader", lines: ["hello", "wörld 😀"] });
  return ops;
}

// ---- the corpus ----
function k7xFiles(dir: string): string[] {
  return readdirSync(dir)
    .sort()
    .flatMap((name) => {
      const abs = join(dir, name);
      if (statSync(abs).isDirectory()) return k7xFiles(abs);
      return name.endsWith(".k7x") ? [abs] : [];
    });
}

const resultText = (r: Result): string =>
  r.e !== undefined ? `E:${r.e}` : `T:${fnv(r.t ?? "")}`;

function corpusEntry(
  name: string,
  text: string,
): { ops: number; hash: string } | null {
  const doc = new K7Document(text);
  if (!doc.contents) return null;
  const data = doc.toJS();
  const ops = opsFor(doc.contents, data, 8);
  const lines = independent(text, ops).map(resultText);
  let printed: string;
  try {
    printed = `T:${fnv(stringify(data))}`;
  } catch (e) {
    printed = `E:${(e as Error).message}`;
  }
  lines.push(printed);
  if (process.env.K7X_VECTORS_DEBUG === name)
    console.error(
      lines.map((l, i) => `${i} ${JSON.stringify(ops[i])} ${l}`).join("\n"),
    );
  return { ops: lines.length, hash: fnv(lines.join("\n")) };
}

// ---- curated cases ----
const cases: unknown[] = [];
const addIndependent = (name: string, text: string, ops: Op[]) =>
  cases.push({
    name,
    text,
    chain: false,
    ops: ops.map((o) => enc(o)),
    results: independent(text, ops),
  });
const addChain = (name: string, text: string, ops: Op[]) => {
  const { results, changes } = chained(text, ops);
  cases.push({
    name,
    text,
    chain: true,
    ops: ops.map((o) => enc(o)),
    results,
    changes,
  });
};

const BASES: [string, string][] = [
  [
    "plain",
    'name("x")\nbpm(120)\nitems([1, 2, 3])\nobj({ a: 1, b: [1, 2] })\n',
  ],
  [
    "crlf",
    'name("x")\r\nitems([\r\n  1,\r\n  2,\r\n])\r\nobj({\r\n  a: 1, // c\r\n  b: 2,\r\n})\r\n',
  ],
  ["bom", '﻿// header one\n// header two\nname("x")\nitems([1, 2])\n'],
  ["tabs", "a({\n\tx: 1,\n\ty: [\n\t\t1,\n\t\t2,\n\t],\n})\nb(1)\n"],
  ["nfd", 'ñame("é")\nitems(["é", "日本", "éé"])\n"k é"(1)\n'],
  ["astral", 'go("😀")\nitems(["😀😀", "a😀"])\nobj({ "😀": 1, b: "x😀" })\n'],
  [
    "chain",
    'track("a").vol(0.5).pan(0)\nfx(a(1).b(2))\nitems([a(1).b(2), c(3)])\n',
  ],
  [
    "chain-multi",
    'track("a")\n  .vol(0.5) // v\n  .pan(0)\n  // note\n  .mute()\nx(1)\n',
  ],
  [
    "comma-first",
    "items([\n  1\n  , 2 // two\n  , 3\n])\nobj({\n  a: 1\n  , b: 2\n})\n",
  ],
  [
    "commented",
    "a([ // c\n])\nb({ /* c */ })\nc([1, /* mid */ 2, 3 /* end */])\nd(1) // tail\n",
  ],
  ["args", "f(1, 2, 3)\ng(1, [2, 3])\nh()\n"],
  ["empty", ""],
  ["only-comments", "// just\n// comments\n"],
  ["no-newline", "a(1)\nb([1, 2])"],
  ["blanks", "\n\na(1)\n\n\nb(2)\n"],
  [
    "bom-inside",
    "a([\ufeff])\nb({ \ufeff})\nc([1,\ufeff])\nd([ // c\n\ufeff])\ne([\ufeff 1 \ufeff])\n",
  ],
  ["null-leaf", "a(null)\nb([null])\n"],
  ["invalid", "a(1\n"],
];

for (const [name, text] of BASES) {
  const doc = new K7Document(text);
  const data = doc.contents ? doc.toJS() : {};
  const ops = doc.contents ? opsFor(doc.contents, data, 24) : [];
  ops.push(
    { op: "setIn", path: ["new"], value: 1 },
    { op: "setIn", path: ["new", "deep", "er"], value: [1, 2] },
    { op: "setIn", path: ["items", 99], value: 1 },
    { op: "setIn", path: ["name", "x"], value: 1 },
    { op: "setIn", path: [], value: { a: 1, b: [1, 2] } },
    { op: "setIn", path: [], value: 5 },
    { op: "addIn", path: ["missing"], value: 1 },
    { op: "addIn", path: ["name"], value: 1 },
    { op: "insertIn", path: ["name"], index: 0, value: 1 },
    { op: "insertIn", path: ["items"], index: 1, value: { a: 1, b: 2 } },
    { op: "insertIn", path: ["items"], index: 0, value: "x".repeat(110) },
    { op: "moveIn", path: ["items"], from: 0, to: 9 },
    { op: "replaceSource", path: ["items"], source: "[9]" },
    { op: "replaceSource", path: [], source: "x" },
    { op: "setSourceIn", path: ["a"], source: "1 // c" },
    { op: "setSourceIn", path: ["a"], source: "bad bad" },
    { op: "setSourceIn", path: [], source: "z(1)\n" },
    { op: "setHeader", lines: [] },
    { op: "setHeader", lines: ["nb\u00a0", "em\u2003", "z\ufeff", "ok "] },
    { op: "setHeader", lines: ["a\r\nb", "", "c  "] },
    { op: "renameKey", path: ["name"], key: "bpm" },
    { op: "renameKey", path: ["name"], key: "n m" },
    { op: "deleteIn", path: ["items", "1"] },
    { op: "deleteIn", path: ["items", "x"] },
    { op: "deleteIn", path: [] },
    { op: "getIn", path: ["items", "1"] },
    { op: "getIn", path: ["nope"] },
    { op: "getSource", path: ["items"] },
    { op: "positionOf", path: ["items"] },
    { op: "positionOf", path: ["items", 1] },
    { op: "positionOf", path: ["obj", "b"] },
    {
      op: "applyIn",
      path: [],
      value: { name: "y", items: [1, 5, 2, 3, 4], extra: { z: 1 } },
    },
    { op: "applyIn", path: ["items"], value: [] },
    { op: "applyIn", path: ["items"], value: [3, 2, 1] },
    { op: "applyIn", path: ["obj"], value: { b: [2], c: null } },
    { op: "applyIn", path: ["gone"], value: [1] },
  );
  addIndependent(`base ${name}`, text, ops);
}

// Widths: a string crossing the 100-column line, in each of the widths JS and UTF-8 count differently.
for (const [label, unit] of [
  ["ascii", "a"],
  ["e-acute", "é"],
  ["cjk", "日"],
  ["astral", "😀"],
  ["nfd", "é"],
] as const) {
  const ops: Op[] = [];
  const u16 = unit.length;
  for (let target = 88; target <= 108; target += 1) {
    const n = Math.max(1, Math.round(target / u16));
    const s = unit.repeat(n);
    ops.push({ op: "setIn", path: ["a"], value: s });
    ops.push({ op: "setIn", path: ["m", "k"], value: s });
    ops.push({ op: "setIn", path: ["l"], value: [s, s] });
    ops.push({ op: "setIn", path: ["l"], value: [1, s] });
    ops.push({ op: "setIn", path: ["m"], value: { [unit.repeat(n)]: 1 } });
    ops.push({ op: "setIn", path: ["m"], value: { x: [s] } });
    ops.push({ op: "addIn", path: ["l"], value: s });
    ops.push({ op: "insertIn", path: ["l"], index: 0, value: { a: s } });
    ops.push({ op: "setIn", path: ["c"], value: { a: s, b: [s] } });
    ops.push({ op: "applyIn", path: [], value: { a: [s, s], b: { [s]: s } } });
  }
  addIndependent(`width ${label}`, "a(1)\nm({})\nl([1])\nc(x(1).y(2))\n", ops);
  const deep: Op[] = [];
  for (let target = 80; target <= 104; target += 2) {
    const s = unit.repeat(Math.max(1, Math.round(target / u16)));
    deep.push({ op: "setIn", path: ["o", "p", "q"], value: [s] });
    deep.push({ op: "setIn", path: ["o", "p", "r"], value: s });
    deep.push({ op: "addIn", path: ["o", "p", "q"], value: s });
    deep.push({ op: "insertIn", path: ["o", "p", "q"], index: 0, value: s });
    deep.push({ op: "setIn", path: ["o", "p"], value: { k: s, l: [s] } });
  }
  addIndependent(
    `width indented ${label}`,
    "o({\n  p: {\n    q: [\n      1,\n    ],\n  },\n})\n",
    deep,
  );
}

// Numbers as JS prints them.
{
  let seed = 0x2545f491;
  const rnd = (): number => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return (seed >>> 0) / 0x100000000;
  };
  const nums: number[] = [
    0,
    -0,
    1,
    -1,
    0.5,
    1.5,
    100,
    1e21,
    1e20,
    123456789012345680000,
    1e-6,
    1e-7,
    1.5e-7,
    5e-324,
    1.7976931348623157e308,
    0.1 + 0.2,
    1 / 3,
    2 ** 53,
    2 ** 53 + 2,
    -1e-10,
    12345.6789,
    4.35,
    0.000001234,
    Number.MAX_SAFE_INTEGER,
    Number.EPSILON,
  ];
  const buf = new DataView(new ArrayBuffer(8));
  for (let i = 0; i < 400; i++) {
    buf.setUint32(0, Math.floor(rnd() * 2 ** 32));
    buf.setUint32(4, Math.floor(rnd() * 2 ** 32));
    const x = buf.getFloat64(0);
    if (Number.isFinite(x)) nums.push(x);
  }
  for (let i = 0; i < 200; i++)
    nums.push(
      Number(
        (rnd() * 10 ** Math.floor(rnd() * 30 - 10)).toPrecision(
          1 + Math.floor(rnd() * 16),
        ),
      ),
    );
  nums.push(Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY);
  addIndependent(
    "numbers",
    "a(1)\n",
    nums.map((value) => ({ op: "setIn", path: ["a"], value })),
  );
  const over = [Number.NaN, Number.POSITIVE_INFINITY, null, 0, "null"];
  addIndependent(
    "a value that prints as null over a null",
    "a(null)\nb(0)\n",
    over.flatMap((value) => [
      { op: "setIn", path: ["a"], value },
      { op: "setIn", path: ["b"], value },
    ]),
  );
  addIndependent("numbers in lists", "a([])\n", [
    { op: "setIn", path: ["a"], value: nums.slice(0, 40) },
    { op: "setIn", path: ["a"], value: [Number.NaN] },
  ]);
}

// Strings as JS prints them.
{
  const strs = [
    "",
    "a",
    'q"uote',
    "back\\slash",
    "tab\t",
    "cr\r",
    "nl\nnl",
    "multi\nline `tick` back\\",
    "ctl\u0001\u001f",
    "del\u007f",
    "line sep",
    "bom﻿",
    "nbsp ",
    "😀",
    "é",
    "日本語",
    "\b\f",
    "$" + "{x}",
    "a\r\nb",
  ];
  addIndependent(
    "strings",
    "a(1)\n",
    strs.map((value) => ({ op: "setIn", path: ["a"], value })),
  );
  addIndependent(
    "strings as keys",
    "m({})\n",
    strs.map((k) => ({ op: "setIn", path: ["m", k], value: 1 })),
  );
  addIndependent(
    "strings as chain keys",
    "l([])\n",
    strs.map((k) => ({ op: "setIn", path: ["l"], value: [{ [k]: 1, b: 2 }] })),
  );
}

// Sugar.
{
  const sugar: SugarSpec = {
    parts: [
      { key: "vol", target: "gain" },
      { key: "pan", target: "pan" },
    ],
  };
  const text = 'vol(0.5)\npan(1)\nname("x")\n';
  const afters: Record<string, unknown>[] = [
    { gain: 0.5, pan: 1, name: "x" },
    { gain: 0.7, pan: 1, name: "x" },
    { gain: 0.5, pan: 2, name: "x" },
    { gain: 0.5, name: "x" },
    { name: "y" },
    { gain: 0.5, pan: 1, name: "x", extra: [1] },
  ];
  addIndependent("sugared", text, [
    ...afters.map((after) => ({ op: "applySugared", after, sugar })),
    ...afters.map((after) => ({ op: "applySugared", after })),
    { op: "applySugared", after: 5 },
  ]);
}

// A random chain of edits on random documents, in three layouts (fuzz.test.ts's generators).
{
  let seed = 1;
  const rnd = (): number => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const pick = <T,>(a: T[]): T => a[Math.floor(rnd() * a.length)];
  const KEYS = ["a", "b", "c", "d-e", "x y", "é", "0", "t", "$k", "😀"];
  const LEAVES: unknown[] = [
    1,
    -2.5,
    "s",
    "L".repeat(45),
    "é\u{1F600}",
    true,
    null,
    "",
    "a\nb",
    "日".repeat(34),
    "😀".repeat(45),
  ];
  type Data = Record<string, unknown>;
  const gen = (depth: number): unknown => {
    const r = rnd();
    if (depth <= 0 || r < 0.4) return pick(LEAVES);
    if (r < 0.7)
      return Array.from({ length: Math.floor(rnd() * 4) }, () =>
        gen(depth - 1),
      );
    const o: Data = {};
    for (let i = Math.floor(rnd() * 4); i > 0; i--)
      o[pick(KEYS)] = gen(depth - 1);
    return o;
  };
  const genRoot = (): Data => {
    const o: Data = {};
    for (let i = Math.floor(rnd() * 5); i > 0; i--) o[pick(KEYS)] = gen(3);
    return o;
  };
  const noisy = (t: string) =>
    t.replace(/,(?=[^"\n]*$)/gm, ", /* c */").replace(/\n/g, " // tail\n");
  const commaFirst = (t: string) =>
    t
      .replace(/,\n([ \t]*)/g, (_, ind) =>
        rnd() < 0.5 ? ` // c\n${ind}, ` : `\n${ind}// mid\n${ind}, `,
      )
      .replace(/,(\s*[\]}])/g, "$1");
  const layout = (data: Data, kind: string): string | null => {
    const src = stringify(data);
    if (kind === "canonical") return src;
    const out = kind === "noisy" ? noisy(src) : commaFirst(src);
    const doc = new K7Document(out);
    if (!doc.contents) return null;
    return JSON.stringify(doc.toJS()) === JSON.stringify(data) ? out : null;
  };
  const paths = (v: unknown, p: Path = []): Path[] => {
    const out = [p];
    if (Array.isArray(v))
      for (const [i, x] of v.entries()) out.push(...paths(x, [...p, i]));
    else if (v && typeof v === "object")
      for (const [k, x] of Object.entries(v)) out.push(...paths(x, [...p, k]));
    return out;
  };
  const getPlain = (o: unknown, p: Path) =>
    p.reduce<unknown>((a, s) => (a as Data)?.[s], o);
  const setPlain = (o: unknown, p: Path, v: unknown): unknown => {
    if (!p.length) return v;
    const [h, ...r] = p;
    const c: Data | unknown[] = Array.isArray(o) ? [...o] : { ...(o as Data) };
    (c as Data)[h] = setPlain((c as Data)[h], r, v);
    return c;
  };
  /** One random op, as the descriptor and the model after it. */
  const next = (model: unknown): { op: Op; model: unknown } | null => {
    const all = paths(model).filter((p) => p.length > 0);
    if (!all.length) return null;
    const p = pick(all);
    const up = p.slice(0, -1);
    const parent = getPlain(model, up);
    const last = p[p.length - 1];
    const kind = Math.floor(rnd() * 7);
    if (kind === 0) {
      const v = gen(2);
      return {
        op: { op: "setIn", path: p, value: v },
        model: setPlain(model, p, v),
      };
    }
    if (kind === 1) {
      const m = Array.isArray(parent)
        ? setPlain(
            model,
            up,
            parent.filter((_, i) => i !== last),
          )
        : (() => {
            const { [last]: _g, ...rest } = parent as Data;
            return setPlain(model, up, rest);
          })();
      return { op: { op: "deleteIn", path: p, prune: rnd() < 0.3 }, model: m };
    }
    if (kind === 2 && Array.isArray(parent)) {
      const v = gen(2);
      const c = [...parent];
      c.splice(last as number, 0, v);
      return {
        op: { op: "insertIn", path: up, index: last, value: v },
        model: setPlain(model, up, c),
      };
    }
    if (kind === 3 && Array.isArray(parent)) {
      const to = Math.floor(rnd() * parent.length);
      const c = [...parent];
      const [moved] = c.splice(last as number, 1);
      c.splice(to, 0, moved);
      return {
        op: { op: "moveIn", path: up, from: last, to },
        model: setPlain(model, up, c),
      };
    }
    if (kind === 4 && !Array.isArray(parent)) {
      const nk = `${String(last)}_r`;
      if (Object.hasOwn(parent as Data, nk)) return null;
      const c: Data = {};
      for (const [k, v] of Object.entries(parent as Data))
        c[k === last ? nk : k] = v;
      return {
        op: { op: "renameKey", path: p, key: nk },
        model: setPlain(model, up, c),
      };
    }
    const cur = getPlain(model, p);
    if (kind === 5 && Array.isArray(cur)) {
      const v = gen(2);
      return {
        op: { op: "addIn", path: p, value: v },
        model: setPlain(model, p, [...cur, v]),
      };
    }
    if (kind === 6) {
      const after = genRoot();
      return { op: { op: "applyIn", path: [], value: after }, model: after };
    }
    return null;
  };
  for (const kind of ["canonical", "noisy", "comma-first"]) {
    const programs: unknown[] = [];
    for (let n = 0; n < 160; n++) {
      const before = genRoot();
      const src = layout(before, kind);
      if (src === null) continue;
      const doc = new K7Document(src);
      let model: unknown = before;
      const ops: Op[] = [];
      for (let k = 0; k < 4; k++) {
        const nx = next(model);
        if (!nx) continue;
        const r = step(doc, nx.op);
        ops.push(nx.op);
        if (r.e !== undefined) break;
        model = nx.model;
      }
      programs.push([src, ops]);
    }
    for (const [i, [src, ops]] of (programs as [string, Op[]][]).entries()) {
      if (i % 4 === 0) addChain(`fuzz ${kind} ${i}`, src, ops);
    }
    // The rest run as one case each, so a failure names a program.
    const rest = (programs as [string, Op[]][]).filter((_, i) => i % 4 !== 0);
    for (const [i, [src, ops]] of rest.entries())
      addChain(`fuzz ${kind} r${i}`, src, ops);
  }
}

// printer: stringify of plain values, headers and inline marks
const printCases: unknown[] = [];
{
  const values: [unknown, { header?: string[]; inline?: Path[] }][] = [
    [{}, {}],
    [{ a: 1 }, { header: ["h"] }],
    [{}, { header: ["h"] }],
    [{ a: [1, 2, 3], b: { c: "x", d: [{ e: 1 }, { f: 2, g: [1] }] } }, {}],
    [{ a: { b: "L".repeat(120) } }, {}],
    [{ a: { b: "L".repeat(120) } }, { inline: [["a"]] }],
    [{ a: ["x".repeat(60), "y".repeat(60)] }, {}],
    [{ a: ["x".repeat(60), "y".repeat(60)] }, { inline: [["a"]] }],
    [{ a: [{ b: 1, c: 2 }, { "d e": 1 }] }, {}],
    [{ a: [{ b: "z".repeat(50), c: "z".repeat(50) }] }, {}],
    [{ "k é": 1, "": 2, "0": 3, "a-b": 4, $x: 5, _y: 6 }, {}],
    [
      { a: "multi\nline", b: ["x\ny", "z"] },
      { header: ["l1\nl2", "", "trailing  "] },
    ],
    [{ a: null, b: true, c: false, d: -0, e: 1e21 }, {}],
    [
      {
        a: {
          b: {
            c: { d: { e: ["é".repeat(30), "日".repeat(30), "😀".repeat(30)] } },
          },
        },
      },
      {},
    ],
    [{ a: [[1, [2, [3, "x".repeat(100)]]]] }, {}],
  ];
  for (const [value, o] of values) {
    const set = new Set((o.inline ?? []).map((p) => JSON.stringify(p)));
    const opts = {
      header: o.header,
      inline: o.inline ? (p: Path) => set.has(JSON.stringify(p)) : undefined,
    };
    let out: { text: string } | { error: string };
    try {
      out = { text: stringify(value, opts) };
    } catch (e) {
      out = { error: (e as Error).message };
    }
    printCases.push({
      value: enc(value),
      header: o.header ?? null,
      inline: o.inline ?? null,
      out,
    });
  }
  for (const bad of [
    5,
    "s",
    null,
    [1],
    { a: Number.NaN },
    { a: [Number.POSITIVE_INFINITY] },
  ]) {
    let out: { text: string } | { error: string };
    try {
      out = { text: stringify(bad) };
    } catch (e) {
      out = { error: (e as Error).message };
    }
    printCases.push({ value: enc(bad), header: null, inline: null, out });
  }
}

// the corpus
const corpus: { file: string; ops: number; hash: string }[] = [];
for (const dir of ["demos", "k7-core"]
  .map((d) => join(ROOT, d))
  .filter(existsSync))
  for (const f of k7xFiles(dir)) {
    const e = corpusEntry(relative(ROOT, f), readFileSync(f, "utf8"));
    if (e)
      corpus.push({
        file: relative(ROOT, f),
        input: fnv(readFileSync(f, "utf8")),
        ...e,
      } as never);
  }

mkdirSync(OUT, { recursive: true });
const dump = (v: unknown) => `${JSON.stringify(v)}\n`;
writeFileSync(join(OUT, "edit.json"), dump({ cases, print: printCases }));
writeFileSync(join(OUT, "corpus.json"), dump(corpus));
void createHash;
void toJS;
console.log(
  `edit vectors: ${cases.length} cases, ${printCases.length} prints, ${corpus.length} corpus files`,
);
