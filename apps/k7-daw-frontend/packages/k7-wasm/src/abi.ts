// abi.ts — loads the WebAssembly module and speaks its C ABI: JSON in, JSON out, two hot calls returning numbers.
// A reader would get wrong that a view of the module's memory dies at the next call: any call may grow it.
// The module is a file beside this one (k7Wasm.wasm): a page streams the hashed asset, node reads it from disk at once.
// There is no TypeScript fallback: without WebAssembly every call throws `unavailable`.

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
interface Compiled {
  module: object;
  instance: Instance;
}
interface WasmApi {
  Module: new (bytes: Uint8Array) => object;
  Instance: new (module: object) => Instance;
  instantiate(bytes: ArrayBuffer): Promise<Compiled>;
  instantiateStreaming?(response: Promise<unknown>): Promise<Compiled>;
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
  k7_round_beat(beats: number): number;
}

/** The module's address: Vite turns this into a hashed asset, node reads it as a file; read lazily, a bundle with no `import.meta.url` never gets here. */
const urlOfModule = () => new URL("./k7Wasm.wasm", import.meta.url);

/** The bytes a host with no asset to fetch hands over (the server's render page bundles them). */
let given: Uint8Array | null = null;

/** Gives the module's bytes before the first call, for a bundle that carries them; see apps/k7-daw-backend/docs/service.md, Picture render. */
export function provideWasmBytes(bytes: Uint8Array): void {
  given = bytes;
}

/** node's file reader, reached without an import a page's bundle would try to resolve. */
const nodeFs = (
  globalThis as {
    process?: { getBuiltinModule?: (id: string) => unknown };
  }
).process?.getBuiltinModule?.("node:fs") as
  | { readFileSync(path: URL): Uint8Array }
  | undefined;

const OK = 0;
const ERROR = 1;
const NONE = 2;

/** The live instance; null before the module is ready and after a trap. */
let abi: Abi | null = null;
let compiled: object | null = null;
let pending: Promise<void> | null = null;
let started = false;
/** Set when no load can succeed in this runtime: calls say so instead of asking for a wait. */
let failed = false;
let epoch = 0;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function start(module: object): Abi {
  return new (wasm as WasmApi).Instance(module).exports as unknown as Abi;
}

/** The page's load: streamed while it downloads, or compiled from the whole body where the server's type refuses streaming. */
async function fetched(api: WasmApi): Promise<Compiled> {
  const response = fetch(urlOfModule());
  if (api.instantiateStreaming) {
    try {
      return await api.instantiateStreaming(response);
    } catch {
      // A server that sends no `application/wasm` refuses the stream, never the bytes.
    }
  }
  return api.instantiate(await (await response).arrayBuffer());
}

/** Loads at once from disk in node; in a page, starts the fetch and leaves `abi` null until it lands. */
function load(): void {
  if (!wasm || started) return;
  started = true;
  if (given || nodeFs) {
    try {
      compiled = new wasm.Module(
        given ??
          (nodeFs as NonNullable<typeof nodeFs>).readFileSync(urlOfModule()),
      );
      abi = start(compiled);
    } catch {
      failed = true;
    }
    return;
  }
  pending = fetched(wasm)
    .then(({ module, instance }) => {
      compiled = module;
      abi = instance.exports as unknown as Abi;
    })
    .catch(() => {
      failed = true;
    });
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
        ? "the WebAssembly module could not be loaded in this runtime"
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

/** A beat rounded to six decimals, by the hot call: no JSON crosses. */
export function roundBeatHot(beats: number): number {
  const x = instance();
  try {
    return x.k7_round_beat(beats);
  } catch (e) {
    if (isTrap(e)) recover(e);
    throw e;
  }
}
