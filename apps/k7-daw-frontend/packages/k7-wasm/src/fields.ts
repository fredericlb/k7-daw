// fields.ts — the editor's field vocabulary, answered by the Rust decoders' own field lists so a completion cannot fall behind a reader.
// A reader would get wrong that a path no kind claims is an empty answer: the editor asks on every keystroke.

import { call } from "./abi.ts";
import type { FileKind } from "./types/index.ts";

export * from "./ops/fields.ts";
export type { FileKind };

/** Which kind a project-relative path is, or null: by FOLDER and not by extension. */
export const fileKindFor = (path?: string): FileKind | null =>
  path ? call<FileKind | null>({ op: "fields.kind", args: { path } }) : null;
