// node.ts — the Worker's runtime and importer for the conformance specs, over the server's routes, in node.
// What a reader gets wrong: it works only as a BUNDLE (scripts/bundleRuntime.mjs), where `node:fs` is the shim; as raw source the reads go to the disk.

import { createImporter, type Importer } from "./importer.ts";
import { type HttpOptions, httpRemote } from "./remote.ts";
import { createRuntime, type Runtime } from "./runtime.ts";

/** A runtime over the server at `options.base`: `open(project)`, then its picture views. */
export function createNodeRunner(options: HttpOptions): Runtime {
  return createRuntime({ remote: httpRemote(options) });
}

/** The import over the server at `options.base`: a project file into a new folder of the workspace the headers name. */
export function createNodeImporter(options: HttpOptions): Importer {
  return createImporter({ remote: httpRemote(options) });
}
