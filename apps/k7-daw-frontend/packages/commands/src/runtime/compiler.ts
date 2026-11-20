// compiler.ts — the compile the verbs reach through `h.project()`: the WebAssembly song module, behind an interface.
// What a reader gets wrong: the module loads on first use, so the Worker's first paint never waits for 3 MB of wasm.

import { FormatTooNewError } from "@studio/song-schema/formatLevel";

export interface Compiled {
  views: Record<string, unknown>;
  reused: boolean;
}

/** What the runtime needs of the compiler: one handle per open project, drafts over it, views out. */
export interface Compiler {
  open(bundle: Record<string, unknown>): number;
  update(handle: number, files: { path: string; text?: string }[]): void;
  /** Throws a NeedsFilesError carrying the paths when a read reached outside the bundle. */
  compile(
    handle: number,
    want: string[],
    o?: { rev?: string; at?: boolean },
  ): Compiled;
  close(handle: number): void;
}

/** The compile read outside the bundle: the paths to fetch before asking again. */
export class CompileNeedsFiles extends Error {
  constructor(readonly files: string[]) {
    super(`the compile reads outside the bundle: ${files.join(", ")}`);
  }
}

/** The WebAssembly compiler, its module loaded on the first call. */
export async function wasmCompiler(): Promise<Compiler> {
  const song = await import("@studio/k7-wasm/song");
  if (!(await song.songReady()))
    throw new Error("WebAssembly is not available in this runtime");
  return {
    open: (b) => song.songOpen(b),
    update: (h, files) => song.songUpdate(h, files),
    compile(h, want, o) {
      try {
        return song.songCompile(h, want as never, o) as Compiled;
      } catch (e) {
        if (e instanceof song.K7WasmError && e.code === "needs_files")
          throw new CompileNeedsFiles(e.files);
        throw e;
      }
    },
    close: (h) => song.songClose(h),
  };
}

interface Failure {
  diagnostics: { file: string; line: number; message: string }[];
  formatTooNew?: { fileLevel: number; buildLevel: number };
}

/** A view that may be a refusal: the value, or the throw a TypeScript compile makes. */
export function viewOf<T>(compiled: Compiled, view: string): T {
  const v = compiled.views[view] as (T & { failure?: Failure }) | undefined;
  if (v === undefined) throw new Error(`the compiler did not answer ${view}`);
  const f = (v as { failure?: Failure } | null)?.failure;
  if (f) {
    if (f.formatTooNew)
      throw new FormatTooNewError(
        f.formatTooNew.fileLevel,
        f.formatTooNew.buildLevel,
      );
    throw new Error(
      f.diagnostics
        .map((d) => (d.file ? `${d.file}:${d.line}: ${d.message}` : d.message))
        .join("\n"),
    );
  }
  return v as T;
}
