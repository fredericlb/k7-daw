// worker.ts — the project's Worker: the picture views' runtime and the importer, made at `init` over the server's routes.
// What a reader gets wrong: nothing in the page imports this file; the bundler builds it as a Worker entry.

import "./prelude.ts";
import { createImporter } from "./importer.ts";
import { serve } from "./protocol.ts";
import { httpRemote } from "./remote.ts";
import { createRuntime } from "./runtime.ts";

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
