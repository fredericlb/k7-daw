// abi.ts — loads the WebAssembly module and speaks its C ABI: JSON in, JSON out, a hot call returning two numbers, and the typed gesture calls (gesture.ts).
// A reader would get wrong that a view of the module's memory dies at the next call: any call may grow it.
// There is no TypeScript fallback: without WebAssembly every call throws `unavailable`.

import { WASM } from "./k7Wasm.generated.ts";
import type { ErrorCode, XY } from "./types/index.ts";

/** What a failed call throws: the Rust side's codes, plus `panic` and `unavailable` raised here. */
export class K7WasmError extends Error {
  readonly code: ErrorCode;
  /** The paths a `needs_files` error asks for. */
  readonly files: string[];
  constructor(code: ErrorCode, message: string, files: string[] = []) {
    super(message);
    this.name = "K7WasmError";
    this.code = code;
    this.files = files;
  }
}

/** The part of WebAssembly this file uses, typed here so it compiles whatever lib a host's tsconfig names. */
interface Instance {
  exports: unknown;
}
interface WasmApi {
  Module: new (bytes: Uint8Array) => object;
  Instance: new (module: object, imports?: object) => Instance;
  instantiate(
    bytes: Uint8Array,
    imports?: object,
  ): Promise<{ module: object; instance: Instance }>;
  RuntimeError: new (message?: string) => Error;
}
const wasm = (globalThis as { WebAssembly?: WasmApi }).WebAssembly;

interface Abi {
  memory: { buffer: ArrayBuffer };
  k7_input(len: number): number;
  k7_call(len: number): number;
  k7_output_ptr(): number;
  k7_output_len(): number;
  k7_curve_playhead(handle: number, beat: number): number;
  k7_scalars_ptr(): number;
  k7_fs_buf(len: number): number;
  k7_f64_in(len: number): number;
  k7_f64_out_ptr(): number;
  /** The gestures' typed calls: numbers in, a number or a status back (docs/gesture.md). */
  [typedCall: `k7_g_${string}`]: (...args: number[]) => number;
}

/** What gesture.ts calls: the instance, its memory and its typed exports. */
export type TypedAbi = Abi;

/** What the imports of a host reach the live instance through. */
export interface HostMemory {
  /** Writes `bytes` into the instance's file buffer, where the module reads them when the import returns. */
  put(bytes: Uint8Array): void;
  /** A copy of the instance's bytes at `ptr`, `len` long (a view dies at the next call). */
  copy(ptr: number, len: number): Uint8Array;
}

type ImportsMaker = (memory: HostMemory) => Record<string, object>;

const OK = 0;
const ERROR = 1;
const NONE = 2;

/** The live instance; null before the first call, after a trap, and while the async compile runs. */
let abi: Abi | null = null;
let compiled: object | null = null;
let pending: Promise<void> | null = null;
let syncTried = false;
/** Set when no compile can succeed in this runtime: calls say so instead of asking for a wait. */
let failed = false;
let epoch = 0;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** The module this runtime loads: the curves' by default, or the one `provide` named before the first call. */
let source: string = WASM;

/** Names the base64 module to load instead (the verbs' Worker's, with the compiler in it); it must come before any call. */
export function provide(base64: string): void {
  if (syncTried || abi)
    throw new Error("provide() comes before the first call");
  source = base64;
}

function bytes(): Uint8Array<ArrayBuffer> {
  const bin = atob(source);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Names the imports the verbs' module asks of its host (its file system, its ids); it must come before the first call. */
let makeImports: ImportsMaker | undefined;
export function provideImports(make: ImportsMaker): void {
  if (syncTried || abi)
    throw new Error("provideImports() comes before the first call");
  makeImports = make;
}

const hostMemory: HostMemory = {
  put(b) {
    const x = abi as Abi;
    const at = x.k7_fs_buf(b.length);
    new Uint8Array(x.memory.buffer, at, b.length).set(b);
  },
  copy(ptr, len) {
    const x = abi as Abi;
    return new Uint8Array(x.memory.buffer, ptr, len).slice();
  },
};

function start(module: object): Abi {
  const imports = makeImports?.(hostMemory);
  return new (wasm as WasmApi).Instance(module, imports)
    .exports as unknown as Abi;
}

/** Compiles synchronously where the runtime allows it, else starts the async compile and leaves `abi` null. */
function load(): void {
  if (!wasm || syncTried) return;
  syncTried = true;
  try {
    compiled = new wasm.Module(bytes());
    abi = start(compiled);
  } catch {
    // A browser's main thread refuses a synchronous compile past a size; wasmReady() awaits the async one.
    pending = Promise.resolve()
      .then(() =>
        (wasm as WasmApi).instantiate(bytes(), makeImports?.(hostMemory)),
      )
      .then(({ module, instance }) => {
        compiled = module;
        abi = instance.exports as unknown as Abi;
      })
      .catch(() => {
        failed = true;
      });
  }
}

/** Resolves when the module is as ready as it will get, true if it can answer; it never rejects. */
export async function wasmReady(): Promise<boolean> {
  load();
  await pending;
  return abi != null;
}

/** Counts the instances thrown away by a trap: a handle opened in an earlier one is gone. */
export function wasmEpoch(): number {
  return epoch;
}

function instance(): Abi {
  load();
  if (abi) return abi;
  throw new K7WasmError(
    "unavailable",
    !wasm
      ? "WebAssembly is not available in this runtime"
      : failed
        ? "the WebAssembly module could not be compiled in this runtime"
        : "the WebAssembly module is not ready: await wasmReady() first",
  );
}

/** Throws the instance away after a trap, since a panic may have left it mid-borrow, and makes a new one. */
function recover(e: unknown): never {
  abi = null;
  epoch++;
  try {
    if (compiled) abi = start(compiled);
  } catch {
    // The next call says `unavailable`.
  }
  const reason = e instanceof Error ? e.message : String(e);
  throw new K7WasmError("panic", `the WebAssembly module trapped: ${reason}`);
}

const isTrap = (e: unknown): boolean =>
  wasm !== undefined && e instanceof wasm.RuntimeError;

/** Runs one request and answers its value; throws K7WasmError for an error status. */
export function call<R = unknown>(request: { op: string; args?: unknown }): R {
  const x = instance();
  const json = JSON.stringify(request);
  let status: number;
  let text: string;
  try {
    const cap = 3 * json.length;
    const at = x.k7_input(cap);
    // The view is made after k7_input and dropped before the next call.
    const view = new Uint8Array(x.memory.buffer, at, cap);
    status = x.k7_call(encoder.encodeInto(json, view).written);
    const out = new Uint8Array(
      x.memory.buffer,
      x.k7_output_ptr(),
      x.k7_output_len(),
    );
    text = decoder.decode(out);
  } catch (e) {
    if (isTrap(e)) recover(e);
    throw e;
  }
  const value = JSON.parse(text) as R;
  if (status === ERROR) {
    const { code, message, files } = value as {
      code: ErrorCode;
      message: string;
      files?: string[];
    };
    throw new K7WasmError(code, message, files);
  }
  return value;
}

/** An open curve's playhead at song beat `beat`: a point, null for none, undefined if the handle is unknown. */
export function playhead(handle: number, beat: number): XY | null | undefined {
  const x = instance();
  try {
    const status = x.k7_curve_playhead(handle, beat);
    if (status === NONE) return null;
    if (status !== OK) return undefined;
    // A fresh view per call: the one read here is the only thing between it and the call.
    const s = new Float64Array(x.memory.buffer, x.k7_scalars_ptr(), 4);
    return { x: s[0], y: s[1] };
  } catch (e) {
    if (isTrap(e)) recover(e);
    throw e;
  }
}

/** The live instance for a typed call; it throws `unavailable` like every call made before the module is ready. */
export function typed(): TypedAbi {
  return abi ?? instance();
}

/** Throws a trap away as `call` does, and rethrows anything else: a typed call's catch hands its error here. */
export function trapped(e: unknown): never {
  if (isTrap(e)) recover(e);
  throw e;
}
