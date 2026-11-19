// native.ts — the Rust build of the measure (rust/), where this runtime has WebAssembly.
// A reader would get wrong that it may differ from envelope.ts: native.test.ts holds every array to the JS one, bit for bit (docs/analysis.md).

import type { Correlations, Measured } from "./envelope.ts";
import { WASM } from "./waveformWasm.generated.ts";

/** One file's measure: samples pushed as they are decoded, measured once, then closed. */
export interface NativeMeasure {
  push(samples: Float32Array, channels: number): void;
  measure(aLow: number, aHigh: number): Measured;
  correlations: Correlations;
  close(): void;
}

/** The part of WebAssembly this file uses, typed here so it compiles whatever lib a host's tsconfig names. */
interface WasmApi {
  Module: new (bytes: Uint8Array) => object;
  Instance: new (module: object, imports: object) => { exports: unknown };
}
const wasm = (globalThis as { WebAssembly?: WasmApi }).WebAssembly;

interface Abi {
  memory: { buffer: ArrayBuffer };
  wf_input(len: number): number;
  wf_push(len: number, channels: number): void;
  wf_measure(aLow: number, aHigh: number): number;
  wf_output_ptr(): number;
  wf_output_len(): number;
  wf_autocorr(len: number, minLag: number, maxLag: number): number;
  wf_reset(): void;
}

/** The six arrays of rust/src/ffi.rs's output, in its order. */
function split(out: Float32Array, buckets: number): Measured {
  const at = (i: number) => out.slice(i * buckets, (i + 1) * buckets);
  return {
    peak: at(0),
    rms: at(1),
    low: at(2),
    mid: at(3),
    high: at(4),
    tempo: out.slice(5 * buckets),
  };
}

let compiled: object | null | undefined;
let kind: "wasm" | "none" = "none";

interface NodeProcess {
  env?: Record<string, string | undefined>;
}
const proc = (globalThis as { process?: NodeProcess }).process;

/** What the module imports: the decoder's file reads (decodeWasm.ts) and the encoders' writes (transcode.ts), which a measure never makes. */
export interface Host {
  read(ptr: number, len: number, pos: number): number;
  write?(ptr: number, len: number, pos: number): number;
}
const NO_FILE: Host = { read: () => -1 };
const NO_OUTPUT = (): number => -1;

/** A fresh instance of the module, with its own memory: one per file, so closing it lets that memory go. */
export function instantiate(module: object, host: Host = NO_FILE): unknown {
  const imports = { read: host.read, write: host.write ?? NO_OUTPUT };
  return new (wasm as WasmApi).Instance(module, { host: imports }).exports;
}

/** The compiled module, or null in a runtime with no WebAssembly. */
export function wasmModule(): object | null {
  if (compiled === undefined) {
    try {
      compiled = wasm ? new wasm.Module(Buffer.from(WASM, "base64")) : null;
    } catch {
      // A JIT-less V8 has no WebAssembly: the JS measure runs instead.
      compiled = null;
    }
  }
  return compiled;
}

function wasmMeasure(module: object): NativeMeasure {
  let x: Abi | null = instantiate(module) as Abi;
  const abi = (): Abi => {
    if (!x) throw new Error("waveform: measure used after close()");
    return x;
  };
  const write = (data: Float32Array): number => {
    const a = abi();
    const at = a.wf_input(data.length);
    new Float32Array(a.memory.buffer, at, data.length).set(data);
    return data.length;
  };
  return {
    push(samples, channels) {
      abi().wf_push(write(samples), channels);
    },
    measure(aLow, aHigh) {
      const a = abi();
      const buckets = a.wf_measure(aLow, aHigh);
      const out = new Float32Array(
        a.memory.buffer,
        a.wf_output_ptr(),
        a.wf_output_len(),
      );
      return split(out, buckets);
    },
    correlations(onset, minLag, maxLag) {
      const a = abi();
      const at = a.wf_autocorr(write(onset), minLag, maxLag);
      return new Float64Array(a.memory.buffer, at, maxLag - minLag + 1).slice();
    },
    close() {
      // The instance and its memory go with it: a two-hour file grows that memory past a gigabyte.
      x = null;
    },
  };
}

/** A measure for one file from the Rust build, or null where this runtime has none (envelope.ts measures then). */
export function openMeasure(): NativeMeasure | null {
  if (proc?.env?.STUDIO_WAVEFORM === "js") return null;
  const module = wasmModule();
  if (!module) return null;
  kind = "wasm";
  return wasmMeasure(module);
}

/** Which build the last measure ran on: what a test and a log say they measured. */
export function measureKind(): typeof kind {
  return kind;
}
