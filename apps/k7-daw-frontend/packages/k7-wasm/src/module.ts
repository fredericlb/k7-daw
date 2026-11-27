// module.ts — how a module is keyed and referenced, and which files a file param can take, answered by the Rust crate (ops/module.rs).
// A reader would get wrong that the tables are here: `FILE_PARAMS` and the library families are generated constants.

import { call } from "./abi.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `module.${op}`, args });

/** The chain KEY of the module a path points into: everything between the type folder and the filename. */
export const moduleKeyFromPath = (path: string): string | null =>
  ask("key_from_path", { path });

/** The ref a library gives the thing at `rest` inside it: `@k7-core/audio/reverb`. */
export const libraryRef = (library: string, rest: string): string =>
  ask("library_ref", { library, rest });

/** The uniform each shader knob lands on: one rule, so a panel and a shader cannot disagree. */
export const shaderUniformFor = (param: string): string =>
  ask("shader_uniform", { param });

/** Those of `files` a param of this name can take; a panel naming its `family` filters by that instead. */
export const assetsForParam = (
  files: readonly string[],
  param: string,
  family?: string,
): string[] =>
  ask("assets_for_param", { files, param, family: family ?? null });
