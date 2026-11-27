// projectName.ts — the folder name a new project will carry. Pure, and kept apart, because that
// name is not a label: the SUFFIX is what decides which application opens it.
// See apps/k7-daw-frontend/packages/web-protocol/projects.ts and ../../docs/projects.md.

import { folderName } from "@studio/k7-wasm/reads";
import {
  PROJECT_SUFFIXES,
  type ProjectApp,
} from "@studio/web-protocol/projects";
import { m } from "../paraglide/messages.js";

export interface ProjectName {
  /** The folder as it will be created, or "" when the name will not do. */
  folder: string;
  /** Why it will not do, or "". */
  error: string;
}

const SUFFIXES = Object.values(PROJECT_SUFFIXES);
// What Rust names a refusal by (song/src/reads/misc.rs), and the message that spells it.
const KEYS: Record<string, "required" | "noSlash" | "noDot"> = {
  required: "required",
  noSlash: "noSlash",
  noDot: "noDot",
};

/** The folder to create, shown BEFORE creating: separators are REFUSED rather than replaced, and a suffix already typed is stripped. */
export function projectFolderName(raw: string, app: ProjectApp): ProjectName {
  const { folder: stem, error } = workspaceFolderName(raw);
  if (error) return { folder: "", error };
  return { folder: `${stem}${PROJECT_SUFFIXES[app]}`, error: "" };
}

/** A dropped file's own name, extension dropped — what the import popin's field starts on. */
export function droppedStem(fileName: string): string {
  return fileName.replace(/\.[^./\\]+$/, "");
}

/** The same name rules, with NO suffix on the end — a workspace is a plain folder, shared so the two dialogs refuse the same things in the same words. */
export function workspaceFolderName(raw: string): ProjectName {
  const { folder, error } = folderName(raw, SUFFIXES);
  return {
    folder,
    error: error ? m[`name.${KEYS[error] ?? "required"}`]() : "",
  };
}
