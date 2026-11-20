// node.ts — the runtime for the CLI and the conformance specs: the same verbs, over the server's routes, in node.
// What a reader gets wrong: it works only as a BUNDLE (scripts/bundleRuntime.mjs), where `node:fs` is the shim; as raw source the verbs read the disk.

import commandsYaml from "../../commands.yaml?raw";
import { provideCommandsYaml } from "../commandSpec.ts";
import { createImporter, type Importer } from "./importer.ts";
import { installPluginVerbs } from "./plugins.ts";
import { type HttpOptions, httpRemote } from "./remote.ts";
import { createRuntime, type Runtime } from "./runtime.ts";

export type { RunResult } from "./runtime.ts";

/** A runtime over the server at `options.base`: `open(project)`, then `run(line)` as many times as needed. */
export function createNodeRunner(options: HttpOptions): Runtime {
  provideCommandsYaml(commandsYaml);
  installPluginVerbs();
  return createRuntime({ remote: httpRemote(options) });
}

/** The import over the server at `options.base`: a project file into a new folder of the workspace the headers name. */
export function createNodeImporter(options: HttpOptions): Importer {
  return createImporter({ remote: httpRemote(options) });
}
