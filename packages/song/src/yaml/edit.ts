// edit.ts — set or clear one field of a workspace file, in place: the FILE primitive, which knows nothing about
// modules or chains (that's packages/song-edit/src/projectEdit.ts). An edit through the k7x door, so every other byte stays.

import type { K7Path } from "@studio/k7x/ast";
import { stub } from "../editStub";

/** Set `path` in `file`, creating the file and any intermediate maps; a short form is kept while it still holds. */
export function setFileValue(file: string, path: K7Path, value: unknown): void {
  stub("song/yaml/edit.setFileValue", file, path, value);
}

/** Remove `path` from `file`. A no-op when it isn't there, or when the file is absent. */
export function deleteFileValue(file: string, path: K7Path): void {
  stub("song/yaml/edit.deleteFileValue", file, path);
}
