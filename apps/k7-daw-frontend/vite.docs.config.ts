// vite.docs.config.ts — the documentation site's build: k7.rocks/docs, a static bundle with no service worker (docs/docsSite.md).

import { defineAppConfig } from "@studio/vite-config";

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
  },
});
