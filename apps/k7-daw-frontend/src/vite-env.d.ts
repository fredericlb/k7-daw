/// <reference types="vite/client" />

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
