// plugins.ts — the builtin plugins' verbs, registered once in this runtime: `track transcribe` is a plugin's, not the core's.
// What a reader gets wrong: the registry refuses a second install, and the node runner is made once per workspace.

import { installBuiltinPlugins } from "@studio/k7-plugins/server";

let installed = false;

export function installPluginVerbs(): void {
  if (installed) return;
  installed = true;
  installBuiltinPlugins();
}
