// song.ts — the compiler in WebAssembly, for the verbs' Worker: a bundle opened into a handle, drafts laid over it, views compiled.
// A reader would get wrong that this loads the verbs' module, not the curves': import it only where the compile is wanted.

import { call, K7WasmError } from "./abi.ts";
import { editReady } from "./edit.ts";

export { K7WasmError };

/** The views a compile can answer, by the names the crate's `View` spells. */
export type SongView =
  | "engine"
  | "web"
  | "errors"
  | "hints"
  | "surfaces"
  | "ioPorts"
  | "catalogue"
  | "baseModules"
  | "graph";

/** The bundle `/verb-bundle` makes (the song crate's `SourceBundle`), libraries merged in. */
export type SongBundle = Record<string, unknown>;

export interface SongCompiled {
  views: Partial<Record<SongView, unknown>>;
  reused: boolean;
}

/** Loads the verbs' module once, with the host's imports it asks for (edit.ts); resolves true when it can answer. */
export function songReady(): Promise<boolean> {
  return editReady();
}

export function songOpen(bundle: SongBundle): number {
  return call<{ handle: number }>({ op: "song.open", args: { bundle } }).handle;
}

/** Lays texts over an open project; a file with no text is removed. */
export function songUpdate(
  handle: number,
  files: { path: string; text?: string }[],
): void {
  call({ op: "song.update", args: { handle, files } });
}

/** Throws a K7WasmError with code `needs_files` and `files` when a read reached outside the bundle. */
export function songCompile(
  handle: number,
  want: SongView[],
  o: { rev?: string; thawed?: string[]; at?: boolean } = {},
): SongCompiled {
  return call<SongCompiled>({
    op: "song.compile",
    args: { handle, want, ...o },
  });
}

export function songClose(handle: number): void {
  call({ op: "song.close", args: { handle } });
}
