// edit.ts — song-edit in WebAssembly: `editCall` runs one exported TypeScript function of the crate, over `node:fs`.
// A reader would get wrong the file system: `node:fs` is the disk in node and the Vfs in the Worker, so one module serves both.

import {
  cpSync,
  type Dirent,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import {
  call,
  K7WasmError,
  provide,
  provideImports,
  wasmReady,
} from "./abi.ts";
import { SONG_WASM } from "./k7WasmSong.generated.ts";

export { K7WasmError };

const OK = 0;
const NOT_FOUND = 1;
const NEEDS = 2;
const IO = 3;

const decoder = new TextDecoder();
const encoder = new TextEncoder();

let tmpCount = 0;
/** A temp beside the file, then a rename: a reader sees the old file or the new one, and a read-only folder refuses. */
function writeAtomic(path: string, bytes: Uint8Array): void {
  let target = path;
  let mode: number | undefined;
  try {
    target = realpathSync(path);
    mode = statSync(target).mode & 0o777;
  } catch {
    // absent: created with the default mode
  }
  const tmp = `${target}.${Math.random().toString(36).slice(2, 8)}.${tmpCount++}.tmp`;
  try {
    writeFileSync(tmp, bytes, mode === undefined ? {} : { mode });
    renameSync(tmp, target);
  } catch (e) {
    rmSync(tmp, { force: true });
    throw e;
  }
}

/** One call: the TypeScript's own function name and its positional arguments. */
export interface EditCall {
  fn: string;
  args: unknown[];
}

export interface EditBatchAnswer {
  rets: unknown[];
  thrown?: string;
}

interface Failure {
  code?: string;
  message?: string;
  name?: string;
}

/** What a crossing costs, counted since the last `editTally(true)`. */
const tally = { calls: 0, imports: 0 };
export function editTally(reset = false): { calls: number; imports: number } {
  const was = { ...tally };
  if (reset) {
    tally.calls = 0;
    tally.imports = 0;
  }
  return was;
}

/** Maps a failed `node:fs` call to the status the module reads; an unknown path is `needs`, never thrown into it. */
function status(put: (b: Uint8Array) => void, e: unknown): number {
  const f = e as Failure;
  if (f.name === "NeedsFiles") return NEEDS;
  if (f.code === "ENOENT") return NOT_FOUND;
  put(encoder.encode(f.message ?? String(e)));
  return IO;
}

let loaded: Promise<boolean> | undefined;

/** Loads the verbs' module once with the host's imports; resolves true when it can answer. */
export function editReady(): Promise<boolean> {
  loaded ??= (async () => {
    provide(SONG_WASM);
    provideImports((mem) => {
      const text = (ptr: number, len: number) =>
        decoder.decode(mem.copy(ptr, len));
      const onPath =
        (run: (p: string) => Uint8Array | undefined) =>
        (ptr: number, len: number): number => {
          const p = text(ptr, len);
          try {
            tally.imports++;
            const out = run(p);
            if (out) mem.put(out);
            return OK;
          } catch (e) {
            return status(mem.put, e);
          }
        };
      return {
        k7: {
          k7_fs_read: onPath((p) => readFileSync(p)),
          k7_fs_stat: onPath((p) => encoder.encode(JSON.stringify(stat(p)))),
          k7_fs_list: onPath((p) => encoder.encode(JSON.stringify(list(p)))),
          k7_fs_real_path: onPath((p) => encoder.encode(realpathSync(p))),
          k7_fs_remove: onPath((p) => void rmSync(p, { force: true })),
          k7_fs_remove_tree: onPath(
            (p) => void rmSync(p, { recursive: true, force: true }),
          ),
          k7_fs_mkdir: onPath((p) => void mkdirSync(p, { recursive: true })),
          k7_fs_write: (pp: number, pn: number, bp: number, bn: number) =>
            onPath((p) => {
              writeHook?.(p);
              writeAtomic(p, mem.copy(bp, bn));
              return undefined;
            })(pp, pn),
          k7_fs_rename: (pp: number, pn: number, qp: number, qn: number) =>
            onPath((p) => {
              renameSync(p, text(qp, qn));
              return undefined;
            })(pp, pn),
          k7_fs_copy: (pp: number, pn: number, qp: number, qn: number) =>
            onPath((p) => {
              cpSync(p, text(qp, qn), { recursive: true });
              return undefined;
            })(pp, pn),
          k7_new_id: () => {
            mem.put(
              encoder.encode(idSource?.() ?? globalThis.crypto.randomUUID()),
            );
            return OK;
          },
        },
      };
    });
    return wasmReady();
  })();
  return loaded;
}

function stat(p: string) {
  const s = statSync(p, { bigint: true } as never) as unknown as {
    size: number | bigint;
    mtimeMs: number | bigint;
    mtimeNs?: bigint;
    isDirectory(): boolean;
  };
  const ms = Number(s.mtimeMs);
  const ns =
    typeof s.mtimeNs === "bigint" ? s.mtimeNs : BigInt(Math.round(ms * 1e6));
  return {
    kind: s.isDirectory() ? "dir" : "file",
    len: Number(s.size),
    mtimeNs: ns.toString(),
    settled: Date.now() - ms > 2000,
  };
}

function list(p: string) {
  return (readdirSync(p, { withFileTypes: true }) as Dirent[]).map((d) => ({
    name: d.name,
    kind: d.isDirectory() ? "dir" : "file",
  }));
}

/** Rethrows what the crate answered as the TypeScript would have thrown it. */
function unwrap<R>(run: () => R): R {
  tally.calls++;
  try {
    return run();
  } catch (e) {
    if (e instanceof K7WasmError && e.code === "edit_failed")
      throw new Error(e.message);
    throw e;
  }
}

/** A binary argument as the crate reads it; JSON carries no bytes. */
export function editBytes(bytes: Uint8Array): { $: "bytes"; b64: string } {
  return { $: "bytes", b64: Buffer.from(bytes).toString("base64") };
}

let writeHook: ((path: string) => void) | undefined;

/** Runs `run` with `hook` called with each path just BEFORE the crate overwrites it (a host's undo wants the old bytes). */
export function withWriteHook<T>(
  hook: ((path: string) => void) | undefined,
  run: () => T,
): T {
  const was = writeHook;
  writeHook = hook;
  try {
    return run();
  } finally {
    writeHook = was;
  }
}

let idSource: (() => string) | undefined;

/** Names where fresh ids come from instead of `randomUUID`; a test that pins ids pins them here. */
export function setIdSource(next: (() => string) | undefined): void {
  idSource = next;
}

/** Runs `fn` (`song-edit/trackMix.editTrackStruct`) over `args`; throws the TypeScript's own message when it threw. */
export function editCall<R = unknown>(fn: string, ...args: unknown[]): R {
  return editCallEnv<R>(undefined, fn, args);
}

/** The same with the environment the TypeScript read from the process (`studioHome`, `metaRoot`, ...). */
export function editCallEnv<R = unknown>(
  env: unknown,
  fn: string,
  args: unknown[],
): R {
  return unwrap(
    () => call<{ ret: R }>({ op: "edit.call", args: { fn, args, env } }).ret,
  );
}

/** Runs the calls in order, stopping at the first throw: one crossing for a loop over a selection. */
export function editBatch(calls: EditCall[], env?: unknown): EditBatchAnswer {
  return unwrap(() =>
    call<EditBatchAnswer>({ op: "edit.batch", args: { calls, env } }),
  );
}
