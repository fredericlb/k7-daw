// folder.ts — whether a path is a folder project: a directory with a project.k7x, or the project file itself.
// A reader would get wrong that this says nothing of the model: a folder that is no graph project is not loaded (project.ts).

import { statSync } from "node:fs";
import { basename } from "node:path";
import { isProjectFolder } from "./yaml/moduleIO";
import { isDocName } from "./yaml/workspaceFile";

// True when `abs` is a folder project (a directory with a project.k7x) or the project file itself.
export function isFolderProject(abs: string): boolean {
  try {
    const st = statSync(abs);
    if (st.isDirectory()) return isProjectFolder(abs);
    return /^project\./i.test(basename(abs)) && isDocName(basename(abs));
  } catch {
    return false;
  }
}
