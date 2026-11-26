// fields.ts — the editor's field vocabulary, answered by the Rust decoders' own field lists so a completion cannot fall behind a reader.
// A reader would get wrong that a path no kind claims is an empty answer: the editor asks on every keystroke.

import { call } from "./abi.ts";
import type { FileKind } from "./types/index.ts";

export type { FileKind };

/** Which kind a project-relative path is, or null: by FOLDER and not by extension. */
export const fileKindFor = (path?: string): FileKind | null =>
  path ? call<FileKind | null>({ op: "fields.kind", args: { path } }) : null;

/** The fields allowed in the mapping at `at`, the editor's key path (`[]` at the top level). */
export const fieldsAt = (path: string, at: string[]): string[] =>
  call<string[]>({ op: "fields.at", args: { path, at } });

/** The values allowed for a field whose schema is a closed set; empty for free text. */
export const valuesAt = (path: string, at: string[], field: string): string[] =>
  call<string[]>({ op: "fields.values", args: { path, at, field } });
