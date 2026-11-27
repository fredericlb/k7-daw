// fileParams.ts — the params whose value is a FILE rather than a number, and the library
// folders such a file may come from. Re-exported by moduleTypes.ts; see docs/moduleTypes.md.
// The tables are the Rust crate's (`@studio/k7-wasm/consts`), and the picks a param takes are `@studio/k7-wasm/module`.

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

export type FileParam = (typeof FILE_PARAMS)[number];

export type SharedFileFamily =
  (typeof SHARED_FILE_PARAMS)[keyof typeof SHARED_FILE_PARAMS];

export type LibraryFileFamily = keyof typeof LIBRARY_FILE_FAMILIES;

/** The library folder a file param can ALSO name a file in, by `@<library>/<path>`; null for the rest. */
export const sharedFamilyOf = (param: string): SharedFileFamily | null =>
  Object.hasOwn(SHARED_FILE_PARAMS, param)
    ? SHARED_FILE_PARAMS[param as keyof typeof SHARED_FILE_PARAMS]
    : null;
export const isFileParam = (name: string): name is FileParam =>
  (FILE_PARAMS as readonly string[]).includes(name);
