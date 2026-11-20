// compileDisk.ts — the song crate's compile (WebAssembly) of a project folder on this disk, for the verbs' tests.
// What a reader gets wrong: the compile reads through the virtual roots of diskBundle.ts, so its answer's paths are spelled back to the real ones.

import { readFileSync, statSync } from "node:fs";
import type { ProjectDoc } from "@studio/engine-protocol/project";
import { listsOf } from "../bundle.ts";
import {
  CompileNeedsFiles,
  type Compiler,
  viewOf,
  wasmCompiler,
} from "../compiler.ts";
import type { BundleFile, Entry } from "../remote.ts";
import { diskBundle, diskLibrary, spellingOf } from "./diskBundle.ts";

let compiler: Compiler | undefined;

/** Loads the WebAssembly module: a test file awaits it once (`beforeAll`), then compiles synchronously. */
export async function compileReady(): Promise<void> {
  compiler ??= await wasmCompiler();
}

/** Spells every string of `v` that is a virtual path back to its real one. */
function unspell(v: unknown, real: (virt: string) => string | null): unknown {
  if (typeof v === "string") return v.startsWith("/") ? (real(v) ?? v) : v;
  if (Array.isArray(v)) return v.map((x) => unspell(x, real));
  if (v && typeof v === "object")
    return Object.fromEntries(
      Object.entries(v).map(([k, x]) => [k, unspell(x, real)]),
    );
  return v;
}

/** The ProjectDoc the engine is sent for the project at `dir`; a refusal throws as the compile's message. */
export function compileDisk(dir: string): ProjectDoc {
  if (!compiler) throw new Error("await compileReady() in beforeAll first");
  const bundle = diskBundle(dir, "r");
  const sp = spellingOf(dir);
  const { files, lists, lengths = {} } = bundle.source;
  const songFiles: Record<string, BundleFile> = { ...files };
  const songLists: Record<string, Entry[]> = { ...lists };
  for (const l of bundle.libraries) {
    const root = sp.real(l.root);
    if (root === null) continue;
    const lib = diskLibrary(l.name, root);
    const rels = Object.keys(lib.files);
    Object.assign(songLists, listsOf(l.root, rels));
    for (const rel of rels)
      songFiles[`${l.root}/${rel}`] = lib.files[rel] as BundleFile;
  }
  const handle = compiler.open({
    ...bundle.source,
    files: songFiles,
    lists: songLists,
    lengths,
  });
  try {
    for (let round = 0; ; round++) {
      try {
        const { doc } = viewOf<{ doc: ProjectDoc }>(
          compiler.compile(handle, ["engine"]),
          "engine",
        );
        return unspell(doc, sp.real) as ProjectDoc;
      } catch (e) {
        if (!(e instanceof CompileNeedsFiles) || round >= 32) throw e;
        compiler.update(
          handle,
          e.files.map((path) => draftOf(path, sp.real)),
        );
      }
    }
  } finally {
    compiler.close(handle);
  }
}

/** What a read outside the bundle finds on this disk: the file's text, or nothing. */
function draftOf(
  path: string,
  real: (virt: string) => string | null,
): { path: string; text?: string } {
  const at = real(path);
  try {
    if (at !== null && statSync(at).isFile())
      return { path, text: readFileSync(at, "utf8") };
  } catch {
    // Absent: the compile is told so, or it asks again.
  }
  return { path };
}
