// worker.ts — the verbs' Worker: commands.yaml bundled in, the runtime made at `init` over the server's routes.
// What a reader gets wrong: nothing in the page imports this file; the bundler builds it as a Worker entry.

import "./prelude.ts";
import commandsYaml from "../../commands.yaml?raw";
import { provideCommandsYaml } from "../commandSpec.ts";
import { createImporter } from "./importer.ts";
import { installPluginVerbs } from "./plugins.ts";
import { serve } from "./protocol.ts";
import { httpRemote } from "./remote.ts";
import { createRuntime } from "./runtime.ts";

provideCommandsYaml(commandsYaml);
installPluginVerbs();

serve(
  self as unknown as Parameters<typeof serve>[0],
  ({ base, headers }, libraries) =>
    createRuntime({
      remote: httpRemote({ base, headers: () => headers ?? {} }),
      libraries,
    }),
  ({ base, headers }, libraries) =>
    createImporter({
      remote: httpRemote({ base, headers: () => headers ?? {} }),
      libraries,
    }),
);
