// atomicWrite.ts — replace a file whole or not at all: write a hidden temp beside it, fsync, rename over.
// The temp's name starts with a dot, so every workspace `skip` (capture, watchers, listings) already hides it.
// Why here, why the Windows retry, and the create-exclusive and retrying-read siblings: docs/atomicWrite.md.

import {
  closeSync,
  fchmodSync,
  fsyncSync,
  linkSync,
  lstatSync,
  openSync,
  readFileSync,
  renameSync,
  statSync,
  unlinkSync,
  writeSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";

const TEMP_NAME = /^\..+\.\d+\.[0-9a-f]{8}\.tmp$/;
const RETRY_CODES = new Set(["EPERM", "EBUSY", "EACCES"]);
const RETRIES = 8;
// A read waits for a Windows holder a few hundred ms at most: a denied file must not freeze its caller (docs/atomicWrite.md).
const READ_RETRIES = 5;

/** Is `name` (a bare file name) one of the temps writeFileAtomic leaves behind if it dies mid-write. */
export const isAtomicTemp = (name: string): boolean => TEMP_NAME.test(name);

/** The temp a write of `file` goes through: hidden, in the same folder, unique to this process and call. */
export function atomicTempFor(file: string): string {
  const rand = Math.floor(Math.random() * 0x100000000)
    .toString(16)
    .padStart(8, "0");
  return join(dirname(file), `.${basename(file)}.${process.pid}.${rand}.tmp`);
}

/** Sleeps `ms` and answers true; answers false at once where the host cannot block, never spinning. */
export function trySleep(ms: number): boolean {
  try {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
    return true;
  } catch {
    return false;
  }
}

/** Sleeps `ms` where the host has Atomics, spins where it has not: a retry's few tens of ms, never a lock's wait. */
export function pause(ms: number): void {
  if (trySleep(ms)) return;
  for (const end = Date.now() + ms; Date.now() < end; ) {
    // spin
  }
}

/** Rename, retrying the transient refusals a Windows scanner or indexer holding `to` gives. */
export function renameRetrying(from: string, to: string): void {
  for (let i = 0; ; i++) {
    try {
      renameSync(from, to);
      return;
    } catch (e) {
      const code = (e as NodeJS.ErrnoException).code ?? "";
      if (
        process.platform !== "win32" ||
        !RETRY_CODES.has(code) ||
        i >= RETRIES
      )
        throw e;
      pause(10 * 2 ** i);
    }
  }
}

/** Reads a text file, retrying the transient refusals another process's rename gives (EBUSY; EPERM, EACCES on Windows). */
export function readTextRetrying(file: string): string {
  for (let i = 0; ; i++) {
    try {
      return readFileSync(file, "utf8");
    } catch (e) {
      const code = (e as NodeJS.ErrnoException).code ?? "";
      const transient =
        code === "EBUSY" ||
        (process.platform === "win32" && RETRY_CODES.has(code));
      if (!transient || i >= READ_RETRIES) throw e;
      pause(10 * 2 ** i);
    }
  }
}

/** A text file's state: its text, absent, or present and unreadable with the errno name. */
export type Read = { text: string } | { absent: true } | { error: string };

export function readText(file: string): Read {
  try {
    return { text: readTextRetrying(file) };
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code ?? "EIO";
    return code === "ENOENT" || code === "ENOTDIR"
      ? { absent: true }
      : { error: code };
  }
}

function modeOf(file: string): number | undefined {
  try {
    return statSync(file).mode & 0o7777;
  } catch {
    return undefined;
  }
}

/** Writes `data` to `file` so a reader sees the old bytes or the new ones, never a part; keeps the file's mode. */
export function writeFileAtomic(
  file: string,
  data: string | Uint8Array,
  opts: { mode?: number; durable?: boolean } = {},
): void {
  writeTemp(file, data, opts, (tmp) => renameRetrying(tmp, file));
}

/** Writes the temp beside `file`, then lets `land` put it in place; the temp is gone after, landed or not. */
function writeTemp(
  file: string,
  data: string | Uint8Array,
  opts: { mode?: number; durable?: boolean },
  land: (tmp: string) => void,
): void {
  const tmp = atomicTempFor(file);
  const kept = modeOf(file);
  const want = kept ?? opts.mode;
  let fd: number | null = null;
  try {
    fd = openSync(tmp, "w", want ?? 0o666);
    // The umask applies to a new file's mode, not to the one an existing file already had.
    if (want !== undefined && process.platform !== "win32")
      fchmodSync(fd, want);
    const bytes = typeof data === "string" ? Buffer.from(data, "utf8") : data;
    let off = 0;
    while (off < bytes.length)
      off += writeSync(fd, bytes, off, bytes.length - off);
    // A cache that is rebuilt after a crash may skip it: it is most of a write's cost on NTFS (docs/atomicWrite.md).
    if (opts.durable !== false) fsyncSync(fd);
    closeSync(fd);
    fd = null;
    land(tmp);
  } catch (e) {
    if (fd !== null) closeSync(fd);
    removeQuietly(tmp);
    throw e;
  }
}

function removeQuietly(file: string): void {
  try {
    unlinkSync(file);
  } catch {
    // never created, or already gone
  }
}

/** Flushes a folder's entries (a rename or a link) to disk; where a folder cannot be opened, there is nothing to flush. */
export function fsyncDir(dir: string): void {
  let fd: number | null = null;
  try {
    fd = openSync(dir, "r");
    fsyncSync(fd);
  } catch {
    // Windows has no folder to flush
  } finally {
    if (fd !== null) closeSync(fd);
  }
}

const NO_LINKS = new Set(["EPERM", "ENOSYS", "ENOTSUP"]);

function present(file: string): boolean {
  try {
    lstatSync(file); // a dangling link is a name taken
    return true;
  } catch {
    return false;
  }
}

/** Creates `file` whole only if the name is free (false when taken); `heldLock` lets a link-less folder publish by rename. */
export function createFileExclusive(
  file: string,
  data: string | Uint8Array,
  opts: { mode?: number; heldLock?: boolean } = {},
): boolean {
  try {
    // A hard link refuses an existing name and shows the file complete or not at all.
    writeTemp(file, data, opts, (tmp) => {
      linkSync(tmp, file);
      removeQuietly(tmp);
    });
    return true;
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code ?? "";
    if (code === "EEXIST") return false;
    if (!NO_LINKS.has(code)) throw e;
  }
  return opts.heldLock
    ? createByRename(file, data, opts)
    : createByOpen(file, data, opts);
}

/** No hard links here: the temp is whole before the name exists, and the name is checked free just before the rename. */
function createByRename(
  file: string,
  data: string | Uint8Array,
  opts: { mode?: number },
): boolean {
  try {
    writeTemp(file, data, opts, (tmp) => {
      if (present(file))
        throw Object.assign(new Error("EEXIST"), { code: "EEXIST" });
      renameRetrying(tmp, file);
    });
    return true;
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "EEXIST") return false;
    throw e;
  }
}

/** No hard links and no lock held: an exclusive open, which a reader may see part-written; only a lock file may use it. */
function createByOpen(
  file: string,
  data: string | Uint8Array,
  opts: { mode?: number },
): boolean {
  if (present(file)) return false;
  let fd: number;
  try {
    fd = openSync(file, "wx", opts.mode ?? 0o666);
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "EEXIST") return false;
    throw e;
  }
  try {
    const bytes = typeof data === "string" ? Buffer.from(data, "utf8") : data;
    for (let off = 0; off < bytes.length; )
      off += writeSync(fd, bytes, off, bytes.length - off);
    fsyncSync(fd);
  } finally {
    closeSync(fd);
  }
  return true;
}
