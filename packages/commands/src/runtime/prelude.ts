// prelude.ts — installs the node globals before any other module of the Worker is evaluated.
// What a reader gets wrong: it must be the FIRST import of worker.ts; song reads `process.env` while its modules load.

import { installGlobals } from "./globals.ts";

installGlobals();
