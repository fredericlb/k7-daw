// fileParams.ts — the params whose value is a FILE rather than a number, and the library
// folders such a file may come from. Re-exported by moduleTypes.ts; see docs/moduleTypes.md.
// The tables are the Rust crate's (`@studio/k7-wasm/consts`); what stays here is the picker's own extensions.

import {
  FILE_PARAMS,
  LIBRARY_FILE_FAMILIES,
  LIBRARY_FILE_PARAMS,
  SHARED_FILE_PARAMS,
  TAP_SAMPLE,
} from "@studio/k7-wasm/consts";

export {
  FILE_PARAMS,
  LIBRARY_FILE_FAMILIES,
  LIBRARY_FILE_PARAMS,
  SHARED_FILE_PARAMS,
  TAP_SAMPLE,
};

/** An amp model is a NAM file or an RTNeural JSON (AIDA-X, Proteus) — apps/k7-daw-engine/k7-runtime/docs/namPlugin.md. */
const PARAM_EXTS: Record<string, readonly string[]> = {
  model: [".nam", ".json"],
};

/** The extensions a file-valued param takes: its table entry, else its NAME — `onnx` loads `.onnx`, see docs/moduleTypes.md. */
export const assetExtsFor = (param: string): readonly string[] =>
  PARAM_EXTS[param] ?? [`.${param.toLowerCase()}`];

/** Those of `files` a param of this name can take — a library pick only the `@<library>/…` sound files; a panel naming its `family` filters by that. */
export function assetsForParam(
  files: string[],
  param: string,
  family?: string,
): string[] {
  // A panel naming a family says what its picker takes, whatever the bound param is called.
  if (family && isLibraryFileFamily(family)) {
    const exts = LIBRARY_FILE_FAMILIES[family];
    return files.filter((f) => exts.some((e) => f.toLowerCase().endsWith(e)));
  }
  if (isLibraryFileParam(param) || param === TAP_SAMPLE)
    return files.filter((f) => f.startsWith("@") && isLibraryAudio(f));
  const exts = assetExtsFor(param);
  return files.filter((f) => exts.some((e) => f.toLowerCase().endsWith(e)));
}
export type FileParam = (typeof FILE_PARAMS)[number];

export type LibraryFileParam = keyof typeof LIBRARY_FILE_PARAMS;
export const isLibraryFileParam = (name: string): name is LibraryFileParam =>
  Object.hasOwn(LIBRARY_FILE_PARAMS, name);

/** The sound files a library's `samples/` and `irs/` hold — what those folders are walked for. */
const LIBRARY_AUDIO_EXTS = LIBRARY_FILE_FAMILIES.samples;
const isLibraryAudio = (path: string): boolean =>
  LIBRARY_AUDIO_EXTS.some((e) => path.toLowerCase().endsWith(e));

export type SharedFileFamily =
  (typeof SHARED_FILE_PARAMS)[keyof typeof SHARED_FILE_PARAMS];

export type LibraryFileFamily = keyof typeof LIBRARY_FILE_FAMILIES;
export const isLibraryFileFamily = (name: string): name is LibraryFileFamily =>
  Object.hasOwn(LIBRARY_FILE_FAMILIES, name);

/** The library folder a file param can ALSO name a file in, by `@<library>/<path>`; null for the rest. */
export const sharedFamilyOf = (param: string): SharedFileFamily | null =>
  Object.hasOwn(SHARED_FILE_PARAMS, param)
    ? SHARED_FILE_PARAMS[param as keyof typeof SHARED_FILE_PARAMS]
    : null;
export const isFileParam = (name: string): name is FileParam =>
  (FILE_PARAMS as readonly string[]).includes(name);
