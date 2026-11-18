// shims/path.ts — node:path for the browser: node's POSIX algorithms, `\` read as `/`.
// What a reader gets wrong: this is never win32 path.

import { current } from "./current.ts";

const cwd = (): string => current().cwd;

export const sep = "/";
export const delimiter = ":";

const slashed = (p: string): string => p.replaceAll("\\", "/");

/** The root a path starts with: `/`, `C:/`, or nothing. */
function rootOf(p: string): string {
  const m = /^[A-Za-z]:\//.exec(p);
  if (m) return m[0];
  return p.startsWith("/") ? "/" : "";
}

export const isAbsolute = (p: string): boolean => rootOf(slashed(p)) !== "";

/** Node's normalizeString: `.` dropped, `..` folded, unless the path is relative and runs out. */
function normalizeString(path: string, allowAboveRoot: boolean): string {
  const out: string[] = [];
  for (const seg of path.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") {
      if (out.length && out[out.length - 1] !== "..") out.pop();
      else if (allowAboveRoot) out.push("..");
    } else out.push(seg);
  }
  return out.join("/");
}

/** A path with nothing to fold: no `.` or `..` segment, no `//`, no `\\`; the interpreter's fast path. */
const FOLDS = /(^|\/)\.\.?(\/|$)|\/\/|\\/;
const clean = (p: string): boolean => p !== "" && !FOLDS.test(p);

export function normalize(p: string): string {
  if (clean(p)) return p;
  const s = slashed(p);
  if (s === "") return ".";
  const root = rootOf(s);
  const trailing = s.endsWith("/");
  let body = normalizeString(s.slice(root.length), root === "");
  if (body === "" && root === "") body = ".";
  if (body !== "" && trailing) body += "/";
  return root + body;
}

export function join(...parts: string[]): string {
  let out = "";
  for (const p of parts) {
    if (p === "") continue;
    if (!clean(p) || (out && p.startsWith("/"))) return slowJoin(parts);
    out = out === "" ? p : out.endsWith("/") ? out + p : `${out}/${p}`;
  }
  return out || ".";
}

function slowJoin(parts: string[]): string {
  const kept = parts.filter((x) => x.length > 0);
  return kept.length ? normalize(kept.join("/")) : ".";
}

export function resolve(...parts: string[]): string {
  const last = parts[parts.length - 1];
  if (last !== undefined && clean(last) && rootOf(last) !== "")
    return last.length > rootOf(last).length && last.endsWith("/")
      ? last.slice(0, -1)
      : last;
  let acc = "";
  for (let i = parts.length - 1; i >= -1 && rootOf(acc) === ""; i--) {
    const p = i >= 0 ? slashed(parts[i] as string) : slashed(cwd());
    if (p.length === 0) continue;
    acc = acc ? `${p}/${acc}` : p;
  }
  const root = rootOf(acc);
  return root + normalizeString(acc.slice(root.length), false) || root || ".";
}

export function dirname(p: string): string {
  const s = slashed(p);
  if (s === "") return ".";
  const root = rootOf(s);
  let end = s.length;
  while (end > root.length && s[end - 1] === "/") end--;
  const cut = s.lastIndexOf("/", end - 1);
  if (cut < root.length) return root || ".";
  return s.slice(0, Math.max(cut, root.length)) || root;
}

export function basename(p: string, ext?: string): string {
  const s = slashed(p);
  let end = s.length;
  while (end > 0 && s[end - 1] === "/") end--;
  const base = s.slice(s.lastIndexOf("/", end - 1) + 1, end);
  return ext && base !== ext && base.endsWith(ext)
    ? base.slice(0, -ext.length)
    : base;
}

export function extname(p: string): string {
  const base = basename(p);
  const dot = base.lastIndexOf(".");
  return dot <= 0 || base === ".." ? "" : base.slice(dot);
}

export function relative(from: string, to: string): string {
  const a = resolve(from);
  const b = resolve(to);
  if (a === b) return "";
  const ra = rootOf(a);
  if (ra !== rootOf(b)) return b;
  const pa = a.slice(ra.length).split("/").filter(Boolean);
  const pb = b.slice(ra.length).split("/").filter(Boolean);
  let i = 0;
  while (i < pa.length && i < pb.length && pa[i] === pb[i]) i++;
  return [...pa.slice(i).map(() => ".."), ...pb.slice(i)].join("/");
}

export function parse(p: string): {
  root: string;
  dir: string;
  base: string;
  ext: string;
  name: string;
} {
  const s = slashed(p);
  const base = basename(s);
  const ext = extname(s);
  return {
    root: rootOf(s),
    dir: s.includes("/") ? dirname(s) : "",
    base,
    ext,
    name: ext ? base.slice(0, -ext.length) : base,
  };
}

const self = {
  sep,
  delimiter,
  isAbsolute,
  normalize,
  join,
  resolve,
  dirname,
  basename,
  extname,
  relative,
  parse,
};
export const posix = self;
export default self;
