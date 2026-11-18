// dev-hosts.mjs — the bit of dev topology that belongs to no single app: the one hostname the dev
// servers answer to besides loopback. A vite dev server refuses a Host it was not told about.
// The dev session itself is a device on loopback (docs/dev.md); only the docs host is a name.

/** The documentation's dev host: k7's dev server answers it with the docs alone, as the hosted k7 does (../docs/dev.md). */
export const DOCS_DEV_HOST = "docs.priv.k7.rocks";
