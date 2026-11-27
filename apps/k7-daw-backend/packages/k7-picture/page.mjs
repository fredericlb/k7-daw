// page.mjs — builds the render page (`@studio/ui-eyes/renderPage`) into one script the server render serves to its
// headless chromium: `node page.mjs <out.js>`, run by the image's build; a checkout runs it once before a render.
// What a reader gets wrong: nothing builds it at run time — apps/k7-daw-backend/docs/service.md, "Picture render".

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../..");
const out = resolve(process.argv[2] ?? join(here, "page", "renderPage.js"));
const r = await build({
  entryPoints: [
    join(repo, "apps/k7-daw-frontend/packages/ui-eyes/src/renderPage.ts"),
  ],
  bundle: true,
  format: "iife",
  platform: "browser",
  loader: { ".wasm": "binary" },
  write: false,
  logLevel: "error",
});
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, r.outputFiles[0]?.text ?? "");
console.log(`render page: ${out}`);
