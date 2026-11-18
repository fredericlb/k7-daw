import { fileURLToPath } from "node:url";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import { studioParaglide } from "@studio/i18n/vite";
import { defineAppConfig } from "@studio/vite-config";
import type { ConfigEnv } from "vite";
import { DOCS_DEV_HOST } from "../../../scripts/dev-hosts.mjs";
import { scaled } from "../../../scripts/vitestScale.ts";
import { noticesPlugin } from "./notices/vitePlugin.ts";
import { entryPreloadPlugin } from "./preload/vitePlugin.ts";
import { verbsWorkerPlugin } from "./verbs/vitePlugin.ts";

/** The paths on this origin that are NOT k7's shell: the worker's scope is `/`, so only this list keeps a navigation to `/api` or `/app_downloads` off it. */
const OTHER_APPS = /^\/(api|auth|app_downloads)(\/|$)/;

/** Heavy build files few sessions open, left out of the precache (packages/vite-config/docs/pwa.md): the docs site (its own host), the About page's notices, the Cyrillic subsets. */
const RARELY_USED = [
  "assets/mountSite-*.js",
  "assets/_virtual_k7-notices-*.js",
  "assets/*-cyrillic*",
  // The verbs' Worker, 3.3 MB gzipped: fetched on first use and kept by the HTTP cache, its names being hashes.
  "assets/verbs/*",
];

/** The language every catalogue resolves the same way, so the plugins' follows k7's — docs/settings.md. */
const STRATEGY: ["localStorage", "baseLocale"] = ["localStorage", "baseLocale"];

/** One module per language for the dev server, which sends each module alone; one per message in a build, so each chunk carries only its own words (docs/settings.md). */
const outputFor = (command: ConfigEnv["command"]) =>
  command === "build" ? "message-modules" : "locale-modules";

/** The builtin plugins' catalogue (the desk, the viewport), compiled where k7-plugins reads it — packages/k7-plugins/docs/i18n.md. */
// fileURLToPath, not `.pathname`: on Windows the latter is `/D:/…`, which a resolve turns into `D:\D:\…`.
const PLUGINS_I18N = fileURLToPath(
  new URL("../../../packages/k7-plugins/", import.meta.url),
);

// Unlike the other apps, k7's dev server has no fixed port: the backend URL it proxies to is env-driven, and always localhost.
const port = Number(process.env.STUDIO_WEB_PORT) || 5223;
const backendPort =
  Number(
    new URL(process.env.STUDIO_BACKEND_URL || "http://localhost:8789").port,
  ) || 8789;

export default ({ command }: ConfigEnv) => {
  const OUTPUT = outputFor(command);
  return defineAppConfig({
    dir: import.meta.url,
    // Dedicated port, and it has to stay unique: `strictPort` makes a clash a hard failure, and this was 5203, which frediprofile owns.
    port,
    backendPort,
    // `/auth` (the local user's session), and the apps' downloads, beside `/api`.
    proxyPaths: ["/auth", "/app_downloads"],
    // The engine relay, live sync's relay and the remote cameras' signalling: only a declared wsPath gets `ws: true` (apps/k7/backend-rs/docs/engine/proxy.md).
    wsPaths: ["/api/engine", "/api/sync", "/api/cameras"],
    // No `base`: k7 is the front door, so its assets and routes sit at `/`. Polling only when asked for (`$STUDIO_POLL`), for a tree native file events cannot see.
    poll: !!process.env.STUDIO_POLL,
    // The one thing about the shared service worker that is k7's: see OTHER_APPS above.
    pwaDenylist: [OTHER_APPS],
    pwaPrecacheIgnore: RARELY_USED,
    extend: {
      // Load-scaled timeouts: the gate sets K7_TEST_TIMEOUT_SCALE on a loaded machine (docs/agent-playbook.md).
      test: { testTimeout: scaled(5_000), hookTimeout: scaled(10_000) },
      // ONE React: a nested `npm install` can leave a copy that shadows the root's, and a pre-bundled dep then calls useContext with no dispatcher.
      resolve: { dedupe: ["react", "react-dom"] },
      // Compiled into src/paraglide/; no `preferredLanguage`, this app defaulting to ENGLISH — docs/settings.md.
      plugins: [
        studioParaglide({ strategy: STRATEGY, outputStructure: OUTPUT }),
        paraglideVitePlugin({
          project: `${PLUGINS_I18N}project.inlang`,
          outdir: `${PLUGINS_I18N}src/paraglide`,
          emitTsDeclarations: true,
          strategy: STRATEGY,
          outputStructure: OUTPUT,
        }),
        // The About page's third-party notices, a chunk of their own — docs/about.md.
        noticesPlugin(),
        // appMain's chunks named in the built page, so they arrive with the entry rather than after it — docs/shell.md.
        entryPreloadPlugin(),
        // The verbs' Worker, built from packages/commands' own bundle — docs/verbs.md.
        verbsWorkerPlugin(),
      ],
      // The docs host still reaches this server — see scripts/dev-hosts.mjs.
      server: {
        allowedHosts: [DOCS_DEV_HOST],
        // HMR by Vite's own port, so a page this Vite serves through another origin — the dev tray's 18789 — reaches it too (docs/dev.md).
        ws: { host: "127.0.0.1", protocol: "ws", clientPort: port },
      },
      // The three @studio UI packages are consumed as raw .ts, and esbuild's dep scanner chokes on the extension in their exports maps.
      optimizeDeps: {
        exclude: [
          "@studio/ui-chains",
          "@studio/ui-chains-react",
          "@studio/ui-editor",
          "@studio/ui-timeline",
        ],
      },
    },
  });
};
