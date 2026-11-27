// workspaceTypes.ts — what the generated wire types leave out: the upload limit, and two names a type goes by.
// A reader would get wrong that the types themselves are not here: they are generated from k7-file-service's wire structs (./generated).

import type { ModuleUiFile } from "./generated";

export type { ChangedFile } from "@studio/k7-wasm/types";

/** A text file beside a module — its DSP source, its widgets' script — shaped like the panel's `ui.k7x`. */
export type ModuleSidecarFile = ModuleUiFile;

/** The largest file one upload carries — the file service's limit, which the browser checks first so it can say so by name. */
export const MAX_UPLOAD_BYTES = 512 * 1024 * 1024;
