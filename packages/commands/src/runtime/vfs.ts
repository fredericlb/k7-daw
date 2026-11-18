// vfs.ts — the in-memory file system the verbs run over: the bundle's texts, binaries as a length, and the writes a run stages.
// What a reader gets wrong: a path under a folder nobody listed is UNKNOWN (NeedsFiles), never absent — docs/runtime.md.

import { textHash } from "@studio/web-protocol/text-hash";

/** A read reached a place the bundle does not cover; the runtime fetches `files` and runs the verb again. */
export class NeedsFiles extends Error {
  constructor(readonly files: string[]) {
    super(`not in the bundle: ${files.join(", ")}`);
    this.name = "NeedsFiles";
  }
}

/** A file the vfs holds: its text, or only its length when it is a binary. */
export interface FileRec {
  text?: string;
  len: number;
  binary?: boolean;
  seconds?: number;
  mtime: number;
  /** The server's staged output this file stands for until the change set lands it. */
  adopted?: { job: string; out: string };
}

export type Kind = 0 | 1;

/** One entry of a staged change set, as `POST /changes` spells an op (paths still the vfs's own). */
export type StagedOp =
  | { kind: "text"; file: string; text: string; was: string | null }
  | { kind: "remove"; file: string; was: string | null }
  | { kind: "copy" | "move"; from: string; file: string; replace?: boolean }
  | {
      kind: "adopt";
      job: string;
      out: string;
      file: string;
      replace?: boolean;
    };

/** What a path held before the run first touched it: its text, `null` for absent, `undefined` for a binary. */
type Origin = string | null | undefined;

export class VfsError extends Error {
  constructor(
    readonly code: string,
    readonly syscall: string,
    readonly path: string,
  ) {
    super(`${code}: ${DESCRIPTIONS[code] ?? "error"}, ${syscall} '${path}'`);
  }
}

const DESCRIPTIONS: Record<string, string> = {
  ENOENT: "no such file or directory",
  EEXIST: "file already exists",
  ENOTDIR: "not a directory",
  EISDIR: "illegal operation on a directory",
  ENOTEMPTY: "directory not empty",
  ENOSYS: "function not implemented",
  EROFS: "binary bytes never cross to the browser",
};

const parentOf = (p: string): string => {
  const i = p.lastIndexOf("/");
  return i <= 0 ? "/" : p.slice(0, i);
};
const inTmp = (p: string): boolean => p === "/tmp" || p.startsWith("/tmp/");
const nameOf = (p: string): string => p.slice(p.lastIndexOf("/") + 1);

/** A posix absolute path with `.`, `..` and doubled slashes folded; a relative one is taken from `cwd`. */
export function normPath(p: string, cwd = "/"): string {
  const out: string[] = [];
  const whole = p.startsWith("/") ? p : `${cwd}/${p}`;
  for (const seg of whole.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") out.pop();
    else out.push(seg);
  }
  return `/${out.join("/")}`;
}

export class Vfs {
  /** A disk, not a mirror: what it does not hold is absent, and every folder lists what it holds (a test server's own). */
  constructor(private readonly disk = false) {}

  readonly files = new Map<string, FileRec>();
  /** A folder's names with their kind, for each folder whose listing is complete. */
  private readonly lists = new Map<string, Map<string, Kind>>();
  /** A folder's names known without a complete listing (the ancestors of what was loaded, new folders). */
  private readonly dirs = new Set<string>(["/"]);
  private readonly origin = new Map<string, Origin>();
  private readonly undo = new Map<string, FileRec | null>();
  private readonly binaryOps: StagedOp[] = [];
  /** The text each file read outside `inside` held, for the `read` ops that make a stale read a 409. */
  readonly reads = new Map<string, string | null>();
  cwd = "/";
  /** The places the server can answer for (the workspace, the libraries): anything else is absent, not unknown. */
  world: string[] = [];
  /** Reads under these folders are the project's own and need no check; everything else read is reported. */
  inside: string[] = [];

  private now(): number {
    return Date.now();
  }

  // --- loading -------------------------------------------------------------

  private link(path: string, kind: Kind): void {
    let at = path;
    let k = kind;
    while (at !== "/") {
      const dir = parentOf(at);
      this.dirs.add(dir);
      const held = this.lists.get(dir);
      if (held && !held.has(nameOf(at))) held.set(nameOf(at), k);
      at = dir;
      k = 1;
    }
  }

  /** Holds a file as the server sent it, whatever the run has staged. */
  load(path: string, rec: Omit<FileRec, "mtime"> & { mtime?: number }): void {
    const p = normPath(path);
    this.files.set(p, { mtime: this.now(), ...rec });
    this.absent.delete(p);
    this.link(p, 0);
  }

  /** Holds a folder's complete listing. */
  loadList(
    dir: string,
    entries: { name: string; kind: "file" | "dir" }[],
  ): void {
    const d = normPath(dir);
    const held = new Map<string, Kind>();
    for (const e of entries) held.set(e.name, e.kind === "dir" ? 1 : 0);
    this.lists.set(d, held);
    this.dirs.add(d);
    this.link(d, 1);
  }

  /** Forgets a file (it moved on the server): the next read of it is unknown until it is loaded again. */
  forget(path: string): void {
    const p = normPath(path);
    this.files.delete(p);
    this.lists.get(parentOf(p))?.delete(nameOf(p));
  }

  /** Whether `dir`'s complete listing is held. */
  listed(dir: string): boolean {
    return this.lists.has(normPath(dir));
  }

  // --- reading -------------------------------------------------------------

  /** Paths whose absence a fetch settled, in folders nobody listed. */
  private readonly absent = new Set<string>();
  /** Every path a read found unknown since the last `takeNeeded`: a verb that swallows the throw is rerun all the same. */
  private needed = new Set<string>();

  private unknown(p: string): never {
    this.needed.add(p);
    throw new NeedsFiles([p]);
  }

  /** The paths found unknown since the last call. */
  takeNeeded(): string[] {
    const out = [...this.needed];
    this.needed = new Set();
    return out;
  }

  /** A fetch found `path` absent: reads say ENOENT from now on. */
  markAbsent(path: string): void {
    this.absent.add(normPath(path));
  }

  stat(path: string): { kind: Kind; size: number; mtime: number } | undefined {
    const p = normPath(path, this.cwd);
    const f = this.files.get(p);
    if (f) return { kind: 0, size: f.len, mtime: f.mtime };
    if (this.lists.has(p) || this.dirs.has(p))
      return { kind: 1, size: 0, mtime: this.now() };
    if (this.absent.has(p)) return undefined;
    if (
      this.world.length > 0 &&
      !this.world.some((w) => p === w || p.startsWith(`${w}/`))
    )
      return undefined;
    const up = this.lists.get(parentOf(p));
    if (up) {
      const kind = up.get(nameOf(p));
      if (kind === undefined) return undefined;
      // Listed and not held: a file the bundle chose not to read, or a folder it did not list.
      return this.unknown(p);
    }
    if (this.disk) return undefined;
    // Below a folder a held listing says is not there, nothing is: the listing above it is the proof.
    for (let a = parentOf(p); a !== "/" && a !== parentOf(a); a = parentOf(a)) {
      const above = this.lists.get(parentOf(a));
      if (above)
        return above.get(nameOf(a)) === undefined ? undefined : this.unknown(p);
    }
    return this.unknown(p);
  }

  private derive(p: string): Map<string, Kind> {
    const names = new Map<string, Kind>();
    const prefix = p === "/" ? "/" : `${p}/`;
    for (const f of this.files.keys())
      if (f.startsWith(prefix)) {
        const rest = f.slice(prefix.length);
        const slash = rest.indexOf("/");
        names.set(slash < 0 ? rest : rest.slice(0, slash), slash < 0 ? 0 : 1);
      }
    for (const d of this.dirs)
      if (d !== p && d.startsWith(prefix)) {
        const rest = d.slice(prefix.length);
        const slash = rest.indexOf("/");
        names.set(slash < 0 ? rest : rest.slice(0, slash), 1);
      }
    return names;
  }

  readdir(path: string): [string, Kind][] {
    const p = normPath(path, this.cwd);
    if (
      this.disk &&
      !this.files.has(p) &&
      !this.dirs.has(p) &&
      !this.lists.has(p)
    )
      throw new VfsError("ENOENT", "scandir", p);
    const held = this.disk
      ? this.files.has(p)
        ? undefined
        : this.derive(p)
      : this.lists.get(p);
    if (!held) {
      if (this.files.has(p)) throw new VfsError("ENOTDIR", "scandir", p);
      if (this.stat(p) === undefined)
        throw new VfsError("ENOENT", "scandir", p);
      return this.unknown(p);
    }
    return [...held.entries()].sort((a, b) =>
      a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0,
    );
  }

  private note(p: string, text: string | null): void {
    if (
      this.reads.has(p) ||
      this.inside.some((r) => p === r || p.startsWith(`${r}/`))
    )
      return;
    if (!this.origin.has(p))
      this.reads.set(p, text === null ? null : textHash(text));
  }

  /** The text of a file; a binary has none to give. */
  readText(path: string): string {
    const p = normPath(path, this.cwd);
    const f = this.files.get(p);
    if (f) {
      if (f.text === undefined) throw new VfsError("EROFS", "open", p);
      this.note(p, f.text);
      return f.text;
    }
    const st = this.stat(p);
    if (!st) {
      this.note(p, null);
      throw new VfsError("ENOENT", "open", p);
    }
    if (st.kind === 1) throw new VfsError("EISDIR", "read", p);
    return this.unknown(p);
  }

  /** A wav's length in seconds as the bundle stated it, or null when none was; an unfetched file is asked for first. */
  secondsOf(path: string): number | null {
    const p = normPath(path, this.cwd);
    const f = this.files.get(p);
    if (f) return f.seconds ?? null;
    if (!this.stat(p)) return null;
    return this.unknown(p);
  }

  textOf(path: string): string | undefined {
    return this.files.get(normPath(path, this.cwd))?.text;
  }

  // --- writing -------------------------------------------------------------

  private touch(p: string): void {
    if (this.origin.has(p)) return;
    const f = this.files.get(p);
    this.origin.set(p, f ? f.text : null);
    this.undo.set(p, f ?? null);
  }

  private needParent(p: string, syscall: string): void {
    const up = parentOf(p);
    if (this.files.has(up)) throw new VfsError("ENOTDIR", syscall, p);
    if (
      !this.dirs.has(up) &&
      !this.lists.has(up) &&
      this.stat(up) === undefined
    )
      throw new VfsError("ENOENT", syscall, p);
  }

  writeText(path: string, text: string, append = false): void {
    const p = normPath(path, this.cwd);
    if (this.stat(p)?.kind === 1) throw new VfsError("EISDIR", "open", p);
    this.needParent(p, "open");
    this.touch(p);
    const held = this.files.get(p);
    if (append && held && held.text === undefined)
      throw new VfsError("EROFS", "open", p);
    const next = append ? (held?.text ?? "") + text : text;
    this.files.set(p, {
      text: next,
      len: new TextEncoder().encode(next).length,
      mtime: this.now(),
    });
    this.link(p, 0);
  }

  mkdir(path: string, recursive: boolean): void {
    const p = normPath(path, this.cwd);
    if (this.files.has(p)) throw new VfsError("EEXIST", "mkdir", p);
    if (this.stat(p)?.kind === 1) {
      if (recursive) return;
      throw new VfsError("EEXIST", "mkdir", p);
    }
    if (!recursive) this.needParent(p, "mkdir");
    // A folder the run made lists as empty and complete.
    const made: string[] = [];
    for (
      let at = p;
      at !== "/" && !this.dirs.has(at) && !this.lists.has(at);
      at = parentOf(at)
    )
      made.push(at);
    for (const d of made) {
      this.lists.set(d, new Map());
      this.dirs.add(d);
    }
    this.link(p, 1);
  }

  private under(p: string): string[] {
    return [...this.files.keys()].filter((f) => f.startsWith(`${p}/`));
  }

  remove(path: string, recursive: boolean): void {
    const p = normPath(path, this.cwd);
    const st = this.stat(p);
    if (!st) throw new VfsError("ENOENT", "rm", p);
    if (st.kind === 1) {
      const below = this.under(p);
      const names = this.lists.get(p);
      if (!recursive && (below.length > 0 || (names?.size ?? 0) > 0))
        throw new VfsError("ENOTEMPTY", "rm", p);
      for (const f of below) this.removeFile(f);
      for (const d of [...this.lists.keys(), ...this.dirs]) {
        if (d !== p && !d.startsWith(`${p}/`)) continue;
        this.lists.delete(d);
        this.dirs.delete(d);
      }
      this.lists.get(parentOf(p))?.delete(nameOf(p));
      return;
    }
    this.removeFile(p);
  }

  private removeFile(p: string): void {
    this.touch(p);
    this.files.delete(p);
    this.lists.get(parentOf(p))?.delete(nameOf(p));
  }

  rename(from: string, to: string): void {
    const a = normPath(from, this.cwd);
    const b = normPath(to, this.cwd);
    const st = this.stat(a);
    if (!st) throw new VfsError("ENOENT", "rename", a);
    if (st.kind === 1) {
      for (const f of this.under(a)) this.rename(f, b + f.slice(a.length));
      this.mkdir(b, true);
      this.remove(a, true);
      return;
    }
    this.place(a, b, "move");
  }

  copy(from: string, to: string): void {
    const a = normPath(from, this.cwd);
    const st = this.stat(a);
    if (!st) throw new VfsError("ENOENT", "copyfile", a);
    if (st.kind === 1) throw new VfsError("EISDIR", "copyfile", a);
    this.place(a, normPath(to, this.cwd), "copy");
  }

  private place(a: string, b: string, how: "copy" | "move"): void {
    const f = this.files.get(a);
    if (!f) this.unknown(a);
    this.needParent(b, how === "move" ? "rename" : "copyfile");
    if (f.text !== undefined) {
      this.writeText(b, f.text);
      if (how === "move" && a !== b) this.removeFile(a);
      return;
    }
    // A binary never crosses the wire: the set names the copy or the move and the server does it.
    const replace = this.files.has(b);
    this.touch(a);
    this.touch(b);
    // An adopted file is not on the server yet: its copy is another adopt, never a `copy` of a file that is not there.
    if (f.adopted) {
      this.files.set(b, { ...f, mtime: this.now() });
      this.link(b, 0);
      if (how === "move" && a !== b) {
        this.files.delete(a);
        this.lists.get(parentOf(a))?.delete(nameOf(a));
      }
      return;
    }
    this.binaryOps.push({
      kind: how,
      from: a,
      file: b,
      ...(replace ? { replace: true } : {}),
    });
    this.files.set(b, { ...f, mtime: this.now() });
    this.link(b, 0);
    if (how === "move" && a !== b) {
      this.files.delete(a);
      this.lists.get(parentOf(a))?.delete(nameOf(a));
    }
  }

  /** Holds a job's staged output at `file`: a binary of that length, landed by an `adopt` op if it is still there at the end. */
  adopt(
    job: string,
    out: string,
    file: string,
    info: { len: number; seconds?: number },
  ): void {
    const p = normPath(file, this.cwd);
    if (this.stat(p)?.kind === 1) throw new VfsError("EISDIR", "open", p);
    this.mkdir(parentOf(p), true);
    this.touch(p);
    this.files.set(p, {
      len: info.len,
      binary: true,
      ...(info.seconds === undefined ? {} : { seconds: info.seconds }),
      adopted: { job, out },
      mtime: this.now(),
    });
    this.link(p, 0);
  }

  // --- the change set ------------------------------------------------------

  /** The paths the run has touched so far. */
  touched(): string[] {
    return [...this.origin.keys()];
  }

  /** What the run changed, as ops: texts that differ from what they held, removals, and the binary copies and moves. */
  staged(): StagedOp[] {
    const ops: StagedOp[] = [...this.binaryOps];
    const named = new Set(
      ops.flatMap((o) =>
        o.kind === "copy" || o.kind === "move"
          ? [o.file, ...(o.kind === "move" ? [o.from] : [])]
          : [],
      ),
    );
    // What is adopted and still stands, outside `/tmp`: a render's scratch copy is not posted.
    for (const [p, f] of this.files) {
      if (!f.adopted || !this.origin.has(p) || inTmp(p)) continue;
      const replace = this.origin.get(p) !== null;
      ops.push({
        kind: "adopt",
        job: f.adopted.job,
        out: f.adopted.out,
        file: p,
        ...(replace ? { replace: true } : {}),
      });
    }
    for (const [p, was] of this.origin) {
      if (named.has(p) && was === undefined) continue;
      const now = this.files.get(p);
      const wasHash = was === undefined || was === null ? null : textHash(was);
      if (!now) {
        if (was !== null) ops.push({ kind: "remove", file: p, was: wasHash });
      } else if (now.text !== undefined && now.text !== was) {
        ops.push({ kind: "text", file: p, text: now.text, was: wasHash });
      }
    }
    return ops;
  }

  /** The run's writes are the server's now: they become the baseline. */
  settle(): void {
    for (const f of this.files.values()) delete f.adopted;
    this.origin.clear();
    this.undo.clear();
    this.binaryOps.length = 0;
    this.reads.clear();
  }

  /** Throws the run's writes away: every touched file goes back to what it held. */
  rollback(): void {
    for (const [p, rec] of this.undo) {
      if (rec) {
        this.files.set(p, rec);
        this.link(p, 0);
      } else {
        this.files.delete(p);
        this.lists.get(parentOf(p))?.delete(nameOf(p));
      }
    }
    this.settle();
  }
}

export { nameOf, parentOf };
