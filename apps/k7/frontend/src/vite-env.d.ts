/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Build stamp "YY.MMDDHHmm", set by the build. Unset in a dev run. */
  readonly VITE_APP_VERSION?: string;
}

/** The third-party notices, built by notices/vitePlugin.ts — ../docs/about.md. */
declare module "virtual:k7-notices" {
  const notices: import("../notices/types").Notices;
  export default notices;
}

/** The verbs' Worker URL, from verbs/vitePlugin.ts — ../docs/verbs.md. */
declare module "virtual:k7-verbs-worker" {
  const url: string;
  export default url;
}
