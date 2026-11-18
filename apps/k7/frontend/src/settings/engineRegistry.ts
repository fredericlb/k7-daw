// engineRegistry.ts — the engine registry as the settings cards read it: this origin's own, with no server's engines.
// The editor's registry (../api.ts) lists the paired server's engines too; the cards list only what this origin holds.

import { engineAccess } from "@studio/engine-browser/access";
import { readJson } from "@studio/http-client/call";
import type { PublicEngine } from "@studio/web-protocol";
import { BASE, type K7Prefs } from "../api";

export const engines = engineAccess<PublicEngine, K7Prefs>(BASE, readJson);
