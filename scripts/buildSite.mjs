// buildSite.mjs — assembles the static site k7.rocks serves from GitHub Pages: the front page, and the
// documentation under /docs (docs/site.md). What a reader gets wrong: every route gets its own index.html,
// since Pages has no SPA fallback for a deep link.

import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const frontend = join(root, "apps/k7/frontend");
const bundle = join(frontend, "dist-docs");
const out = resolve(process.argv[2] ?? join(root, "dist-site"));

/** The documentation's routes, as src/docs/site/pages.ts derives them from docs-site/. */
function routes(dir = join(root, "docs-site"), prefix = "") {
  const found = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory())
      found.push(...routes(join(dir, entry.name), `${prefix}${entry.name}/`));
    else if (entry.name.endsWith(".md")) {
      const stem = `${prefix}${entry.name.slice(0, -3)}`;
      found.push(stem.replace(/(^|\/)00-index$/, "").replace(/^index$/, ""));
    }
  }
  return found;
}

const build = spawnSync(
  "npx",
  ["vite", "build", "--config", "vite.docs.config.ts"],
  { cwd: frontend, stdio: "inherit" },
);
if (build.status !== 0) process.exit(build.status ?? 1);

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "docs"), { recursive: true });
cpSync(join(root, "site"), out, { recursive: true });
cpSync(join(frontend, "public/favicon.ico"), join(out, "favicon.ico"));
cpSync(join(frontend, "icon/k7-splash.svg"), join(out, "k7-splash.svg"));
cpSync(bundle, join(out, "docs"), { recursive: true });

const page = join(out, "docs/index.html");
for (const route of routes().filter(Boolean)) {
  const dir = join(out, "docs", route);
  mkdirSync(dir, { recursive: true });
  cpSync(page, join(dir, "index.html"));
}
cpSync(page, join(out, "404.html"));
for (const f of readdirSync(join(out, "docs"), { recursive: true }))
  if (/\.(gz|br)$/.test(f)) rmSync(join(out, "docs", f));
writeFileSync(join(out, "CNAME"), "k7.rocks\n");
writeFileSync(join(out, ".nojekyll"), "");
console.log(`site built into ${out}`);
