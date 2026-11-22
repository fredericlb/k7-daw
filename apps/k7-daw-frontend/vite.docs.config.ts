// vite.docs.config.ts — the documentation site's build: k7.rocks/docs, a static bundle with no service worker (docs/docsSite.md).

import { fileURLToPath } from "node:url";
import { studioParaglide } from "@studio/i18n/vite";
import { defineAppConfig } from "@studio/vite-config";

/** This folder, whole: the root is `docs-entry`, and the catalogue's relative paths would resolve from there. */
const HERE = fileURLToPath(new URL("./", import.meta.url));

export default defineAppConfig({
  dir: import.meta.url,
  port: 5224,
  backendPort: 8789,
  base: "/docs/",
  pwa: false,
  extend: {
    // The entry's own folder is the root, so its `index.html` is the site's and nothing of the editor's is served.
    root: "docs-entry",
    publicDir: false,
    build: { outDir: "../dist-docs", emptyOutDir: true },
    resolve: { dedupe: ["react", "react-dom"] },
    // The site's words, compiled into src/paraglide/ as the editor's build does: a fresh checkout has none.
    plugins: [
      studioParaglide({
        project: `${HERE}project.inlang`,
        outdir: `${HERE}src/paraglide`,
        strategy: ["localStorage", "baseLocale"],
        outputStructure: "message-modules",
      }),
    ],
  },
});
