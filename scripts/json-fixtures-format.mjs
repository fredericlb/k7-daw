#!/usr/bin/env node
// Formats the committed JSON fixtures: one key per line, small containers and scalar arrays on one line.
// What a reader gets wrong: literals are kept as written (numbers, escapes), so the file's values never change.
// Usage: json-fixtures-format.mjs [--check] <file|dir>...   (rewrites in place; --check only reports)
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const INLINE_MAX = 160;
const FLAT_MAX = 240;

// A long array of scalars fills lines up to INLINE_MAX, a long string on a line of its own.
function wrap(raws, pad) {
  const lines = [];
  let line = "";
  for (const r of raws) {
    if (line && pad.length + line.length + r.length + 2 > INLINE_MAX) {
      lines.push(pad + line);
      line = "";
    }
    line += line ? `, ${r}` : r;
  }
  lines.push(pad + line);
  return lines.join(",\n");
}

// A tiny parser that keeps number and string literals verbatim.
function parse(src) {
  let i = 0;
  const ws = () => {
    while (i < src.length && " \t\r\n".includes(src[i])) i++;
  };
  const value = () => {
    ws();
    const c = src[i];
    if (c === "{") {
      i++;
      const entries = [];
      ws();
      if (src[i] === "}") {
        i++;
        return { t: "o", entries };
      }
      for (;;) {
        ws();
        const k = value();
        ws();
        i++; // the colon
        entries.push([k.raw, value()]);
        ws();
        if (src[i++] === "}") return { t: "o", entries };
      }
    }
    if (c === "[") {
      i++;
      const items = [];
      ws();
      if (src[i] === "]") {
        i++;
        return { t: "a", items };
      }
      for (;;) {
        items.push(value());
        ws();
        if (src[i++] === "]") return { t: "a", items };
      }
    }
    const start = i;
    if (c === '"') {
      i++;
      while (src[i] !== '"') i += src[i] === "\\" ? 2 : 1;
      i++;
    } else {
      while (i < src.length && !",]} \t\r\n".includes(src[i])) i++;
    }
    return { t: "s", raw: src.slice(start, i) };
  };
  const v = value();
  ws();
  if (i !== src.length) throw new Error("trailing content");
  return v;
}

const scalar = (n) => n.t === "s";
const flat = (n) =>
  (n.t === "a" ? n.items : n.entries.map((e) => e[1])).every(scalar);

function compact(n) {
  if (scalar(n)) return n.raw;
  if (n.t === "a") return `[${n.items.map(compact).join(", ")}]`;
  return `{${n.entries.map(([k, v]) => `${k}: ${compact(v)}`).join(", ")}}`;
}

function print(n, unit, depth, room) {
  if (scalar(n)) return n.raw;
  const empty = n.t === "a" ? !n.items.length : !n.entries.length;
  if (empty) return n.t === "a" ? "[]" : "{}";
  const one = compact(n);
  if (one.length <= (flat(n) ? FLAT_MAX : room)) return one;
  const pad = unit.repeat(depth + 1);
  const end = unit.repeat(depth);
  if (n.t === "a" && flat(n))
    return `[\n${wrap(
      n.items.map((v) => v.raw),
      pad,
    )}\n${end}]`;
  const inner = Math.max(40, INLINE_MAX - (depth + 1) * unit.length);
  const lines =
    n.t === "a"
      ? n.items.map((v) => pad + print(v, unit, depth + 1, inner))
      : n.entries.map(
          ([k, v]) =>
            `${pad}${k}: ${print(v, unit, depth + 1, inner - k.length - 2)}`,
        );
  return `${n.t === "a" ? "[" : "{"}\n${lines.join(",\n")}\n${end}${n.t === "a" ? "]" : "}"}`;
}

export function format(src, unit) {
  const out = print(parse(src), unit, 0, INLINE_MAX);
  return src.endsWith("\n") ? `${out}\n` : out;
}

export function indentOf(src) {
  const m = src.match(/\n( +)\S/);
  return m ? m[1] : "  ";
}

function files(p) {
  if (!statSync(p).isDirectory()) return [p];
  return readdirSync(p).flatMap((f) => files(join(p, f)));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const check = args.includes("--check");
  let stale = 0;
  for (const f of args
    .filter((a) => a !== "--check")
    .flatMap(files)
    .filter((f) => f.endsWith(".json"))) {
    const src = readFileSync(f, "utf8");
    const out = format(src, indentOf(src));
    if (out === src) continue;
    stale++;
    if (check) console.log(`not formatted: ${f}`);
    else writeFileSync(f, out);
  }
  if (check && stale) process.exit(1);
}
