// curve.ts — a curve's picture from Rust: one call for a draft, a handle for a card that is drawn every frame.
// A reader would get wrong that a handle is a hint: it reopens itself from the spec it kept, so a missed close never breaks it.

import { call, K7WasmError, playhead, wasmEpoch } from "./abi.ts";
import type { CurvePreview, CurveSpec, XY } from "./types/index.ts";

export { K7WasmError };

/** A curve held open in the module: its picture once, and its playhead every frame without resolving it again. */
export class CurveHandle {
  #handle: number | null = null;
  #epoch = -1;
  #picture: CurvePreview | null = null;

  constructor(
    readonly spec: CurveSpec,
    readonly num: number,
    readonly den: number,
    readonly n?: number,
  ) {}

  #open(): { handle: number; preview: CurvePreview } {
    const { spec, num, den, n } = this;
    const opened = call<{ handle: number; preview: CurvePreview }>({
      op: "curve.open",
      args: { spec, num, den, n },
    });
    this.#handle = opened.handle;
    this.#epoch = wasmEpoch();
    this.#picture = opened.preview;
    return opened;
  }

  /** The handle, opened if it never was, was closed, or died with an instance that trapped. */
  #live(): number {
    if (this.#handle === null || this.#epoch !== wasmEpoch())
      return this.#open().handle;
    return this.#handle;
  }

  /** The picture, the same object on every read. */
  get preview(): CurvePreview {
    return this.#picture ?? this.#open().preview;
  }

  /** Where song beat `beat` falls on the picture; null for an envelope or a beat before the start. */
  playhead(beat: number): XY | null {
    const first = playhead(this.#live(), beat);
    if (first !== undefined) return first;
    // The slot was evicted or reused: open again from the spec and ask once more.
    return playhead(this.#open().handle, beat) ?? null;
  }

  /** Frees the slot; the handle opens itself again if it is used after. */
  close(): void {
    const handle = this.#handle;
    this.#handle = null;
    if (handle === null || this.#epoch !== wasmEpoch()) return;
    try {
      call({ op: "curve.close", args: { handle } });
    } catch (e) {
      // An unknown handle means the slot is already free.
      if (!(e instanceof K7WasmError)) throw e;
    }
  }
}
