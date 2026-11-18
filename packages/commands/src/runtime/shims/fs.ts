// shims/fs.ts — node:fs's synchronous calls over the Vfs; errors carry node's `code`, `syscall` and `path`.
// What a reader gets wrong: a read of something the bundle does not cover throws NeedsFiles, not ENOENT — vfs.ts.

import { NeedsFiles, normPath, VfsError } from "../vfs.ts";
import { current } from "./current.ts";
import { dirname, join } from "./path.ts";

export { NeedsFiles };

type NodeError = Error & { code: string; syscall: string; path: string };

/** Turns the vfs's own error into the one node throws; NeedsFiles passes through untouched. */
function guard<T>(f: () => T): T {
  try {
    return f();
  } catch (e) {
    if (e instanceof VfsError) {
      const n = new Error(e.message) as NodeError;
      n.code = e.code;
      n.syscall = e.syscall;
      n.path = e.path;
      throw n;
    }
    throw e;
  }
}

function fail(code: string, syscall: string, path: string): never {
  return guard(() => {
    throw new VfsError(code, syscall, path);
  });
}

export class Stats {
  readonly size: number;
  readonly mtimeMs: number;
  readonly ctimeMs: number;
  readonly birthtimeMs: number;
  readonly atimeMs: number;
  readonly mode: number;
  constructor(
    private readonly kind: 0 | 1,
    size: number,
    mtime: number,
  ) {
    this.size = size;
    this.mtimeMs = this.ctimeMs = this.birthtimeMs = this.atimeMs = mtime;
    this.mode = kind === 1 ? 0o755 : 0o644;
  }
  get mtime(): Date {
    return new Date(this.mtimeMs);
  }
  get ctime(): Date {
    return new Date(this.mtimeMs);
  }
  get birthtime(): Date {
    return new Date(this.mtimeMs);
  }
  get atime(): Date {
    return new Date(this.mtimeMs);
  }
  isFile = (): boolean => this.kind === 0;
  isDirectory = (): boolean => this.kind === 1;
  isSymbolicLink = (): boolean => false;
}

interface StatOpts {
  throwIfNoEntry?: boolean;
}

function statOf(
  path: string,
  syscall: string,
  opts?: StatOpts,
): Stats | undefined {
  const p = String(path);
  const r = current().stat(p);
  if (!r) {
    if (opts?.throwIfNoEntry === false) return undefined;
    return fail("ENOENT", syscall, p);
  }
  return new Stats(r.kind, r.size, r.mtime);
}

export const statSync = (path: string, opts?: StatOpts): Stats =>
  statOf(path, "stat", opts) as Stats;
export const lstatSync = (path: string, opts?: StatOpts): Stats =>
  statOf(path, "lstat", opts) as Stats;

export function existsSync(path: unknown): boolean {
  if (typeof path !== "string" || path === "") return false;
  return current().stat(path) !== undefined;
}

export class Dirent {
  constructor(
    readonly name: string,
    private readonly kind: 0 | 1,
    readonly parentPath: string,
  ) {}
  get path(): string {
    return this.parentPath;
  }
  isFile = (): boolean => this.kind === 0;
  isDirectory = (): boolean => this.kind === 1;
  isSymbolicLink = (): boolean => false;
}

interface ReaddirOpts {
  withFileTypes?: boolean;
  recursive?: boolean;
}

export function readdirSync(
  path: string,
  opts?: ReaddirOpts | string,
): (string | Dirent)[] {
  const o = typeof opts === "object" ? opts : {};
  const out: (string | Dirent)[] = [];
  const walk = (dir: string, rel: string): void => {
    for (const [name, kind] of guard(() => current().readdir(dir))) {
      const r = rel ? `${rel}/${name}` : name;
      out.push(o.withFileTypes ? new Dirent(name, kind, dir) : r);
      if (o.recursive && kind === 1) walk(join(dir, name), r);
    }
  };
  walk(String(path), "");
  return out;
}

const encodingOf = (opts: unknown): string | undefined =>
  typeof opts === "string"
    ? opts
    : ((opts as { encoding?: string } | undefined)?.encoding ?? undefined);

const encoder = new TextEncoder();

export function readFileSync(path: string | number, opts?: unknown): unknown {
  const p = typeof path === "number" ? descriptor(path).path : String(path);
  const text = guard(() => current().readText(p));
  const enc = encodingOf(opts);
  if (enc === "utf8" || enc === "utf-8") return text;
  const bytes = encoder.encode(text);
  const B = (
    globalThis as {
      Buffer?: { from(b: Uint8Array): { toString(e: string): string } };
    }
  ).Buffer;
  const buf = B ? B.from(bytes) : bytes;
  return enc && B
    ? (buf as { toString(e: string): string }).toString(enc)
    : buf;
}

/** A write's text: a string as is; bytes are decoded, since a browser stages texts only (a binary is a copy or a move). */
function payload(data: unknown, path: string): string {
  if (typeof data === "string") return data;
  if (ArrayBuffer.isView(data)) {
    const bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    return textOrRefuse(bytes, path);
  }
  if (data instanceof ArrayBuffer)
    return textOrRefuse(new Uint8Array(data), path);
  return String(data);
}

function textOrRefuse(bytes: Uint8Array, path: string): string {
  try {
    // The BOM is kept: a file the editor read with one is written back with it.
    return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(
      bytes,
    );
  } catch {
    return fail("EROFS", "open", path);
  }
}

export function writeFileSync(
  path: string,
  data: unknown,
  opts?: unknown,
): void {
  const flag = (opts as { flag?: string } | undefined)?.flag ?? "w";
  const p = String(path);
  guard(() => current().writeText(p, payload(data, p), flag.startsWith("a")));
}

export function appendFileSync(path: string, data: unknown): void {
  const p = String(path);
  guard(() => current().writeText(p, payload(data, p), true));
}

export function mkdirSync(
  path: string,
  opts?: { recursive?: boolean } | number,
): string | undefined {
  const recursive = typeof opts === "object" && !!opts.recursive;
  const existed = existsSync(path);
  guard(() => current().mkdir(String(path), recursive));
  return recursive && !existed ? String(path) : undefined;
}

export function rmSync(
  path: string,
  opts?: { recursive?: boolean; force?: boolean },
): void {
  try {
    guard(() => current().remove(String(path), !!opts?.recursive));
  } catch (e) {
    if ((e as NodeError).code === "ENOENT" && opts?.force) return;
    throw e;
  }
}

export const rmdirSync = (path: string): void =>
  guard(() => current().remove(String(path), false));
export const unlinkSync = (path: string): void => {
  if (statOf(path, "unlink")?.isDirectory())
    fail("EISDIR", "unlink", String(path));
  guard(() => current().remove(String(path), false));
};
export const renameSync = (from: string, to: string): void =>
  guard(() => current().rename(String(from), String(to)));
export const copyFileSync = (from: string, to: string): void =>
  guard(() => current().copy(String(from), String(to)));

export function cpSync(
  from: string,
  to: string,
  opts?: { recursive?: boolean; filter?: (s: string, d: string) => boolean },
): void {
  if (opts?.filter && !opts.filter(from, to)) return;
  if (!statSync(from).isDirectory()) {
    mkdirSync(dirname(to), { recursive: true });
    copyFileSync(from, to);
    return;
  }
  mkdirSync(to, { recursive: true });
  for (const [name] of guard(() => current().readdir(String(from))))
    cpSync(join(from, name), join(to, name), opts);
}

export function realpathSync(path: string): string {
  const p = normPath(String(path), current().cwd);
  if (!current().stat(p)) fail("ENOENT", "realpath", String(path));
  return p;
}
realpathSync.native = realpathSync;

export const utimesSync = (
  _path: string,
  _atime: number | Date,
  _mtime: number | Date,
): void => {};

let tempSeq = 0;
export function mkdtempSync(prefix: string): string {
  const dir = `${prefix}${Math.random().toString(36).slice(2, 8)}${tempSeq++}`;
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function symlinkSync(_target: string, path: string): void {
  fail("ENOSYS", "symlink", String(path));
}

/** An open file: its path and where a read with no position goes on from. */
interface Descriptor {
  path: string;
  pos?: number;
}
const descriptors = new Map<number, Descriptor>();
let nextFd = 3;

function descriptor(fd: number): Descriptor {
  const d = descriptors.get(fd);
  if (!d) {
    const e = new Error("EBADF: bad file descriptor") as NodeError;
    e.code = "EBADF";
    throw e;
  }
  return d;
}

export function openSync(path: string, flags: string | number = "r"): number {
  const p = String(path);
  const f = typeof flags === "number" ? "r" : flags;
  if (f.includes("x")) {
    if (current().stat(p)) fail("EEXIST", "open", p);
    guard(() => current().writeText(p, "", false));
  } else if (f.startsWith("w")) guard(() => current().writeText(p, "", false));
  else if (f.startsWith("a")) guard(() => current().writeText(p, "", true));
  else statOf(p, "open");
  const fd = nextFd++;
  descriptors.set(fd, { path: p });
  return fd;
}

export function writeSync(fd: number, data: unknown): number {
  const d = descriptor(fd);
  const text = payload(data, d.path);
  guard(() => current().writeText(d.path, text, true));
  return typeof data === "string"
    ? data.length
    : (data as ArrayBufferView).byteLength;
}

export function readSync(
  fd: number,
  buf: Uint8Array,
  offset = 0,
  length = buf.byteLength - offset,
  position: number | null = null,
): number {
  const d = descriptor(fd);
  const bytes = encoder.encode(guard(() => current().readText(d.path)));
  const from = position ?? d.pos ?? 0;
  const chunk = bytes.subarray(from, from + length);
  buf.set(chunk, offset);
  if (position == null) d.pos = from + chunk.byteLength;
  return chunk.byteLength;
}

export const fstatSync = (fd: number): Stats => statSync(descriptor(fd).path);
export const closeSync = (fd: number): void => {
  descriptors.delete(fd);
};
export const fsyncSync = (fd: number): void => {
  descriptor(fd);
};
export const fchmodSync = (fd: number, _mode: number | string): void => {
  descriptor(fd);
};
export const chmodSync = (_path: string, _mode: number | string): void => {};
export const linkSync = (from: string, to: string): void => {
  if (current().stat(String(to))) fail("EEXIST", "link", String(to));
  copyFileSync(from, to);
};

export const constants = { F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1 };

export default {
  statSync,
  lstatSync,
  existsSync,
  readdirSync,
  readFileSync,
  writeFileSync,
  appendFileSync,
  mkdirSync,
  rmSync,
  rmdirSync,
  unlinkSync,
  renameSync,
  copyFileSync,
  cpSync,
  realpathSync,
  utimesSync,
  mkdtempSync,
  symlinkSync,
  openSync,
  writeSync,
  readSync,
  fstatSync,
  closeSync,
  fsyncSync,
  fchmodSync,
  chmodSync,
  linkSync,
  constants,
};
