// rustSources.mjs — the hashes that tell a stale src/k7Wasm.wasm: every Rust source, and the generated types and constants.
// A reader would get wrong what SOURCES covers: each crate's manifest and src, what they include, and the two locks, line endings folded.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const ROOT = join(import.meta.dirname, "..", "..", "..", "..", "..");
const PKG = join(import.meta.dirname, "..");

/** The crates the module is built from, from the repo root; the facade first, the one whose lock counts. */
export const CRATES = [
  "apps/k7-daw-wasm/k7-wasm",
  "apps/k7-daw-wasm/song-schema",
  "apps/k7-daw-wasm/song",
  "apps/k7-daw-wasm/song-edit",
  "apps/k7-daw-wasm/k7x",
  "apps/k7-daw-wasm/verbs",
  "apps/k7-daw-wasm/eyes",
];

function files(dir) {
  return readdirSync(dir)
    .sort()
    .flatMap((name) => {
      const abs = join(dir, name);
      return statSync(abs).isDirectory() ? files(abs) : [abs];
    });
}

function hashOf(paths, base) {
  const hash = createHash("sha256");
  for (const f of paths) {
    const rel = relative(base, f).replaceAll("\\", "/");
    hash.update(
      `${rel}\0${readFileSync(f, "utf8").replaceAll("\r\n", "\n")}\0`,
    );
  }
  return hash.digest("hex").slice(0, 16);
}

/** The locks that pin what the hashed crates build against: the facade's, and the compiler's, which holds `song` and `k7x`. */
const LOCKS = [
  "apps/k7-daw-wasm/k7-wasm/Cargo.lock",
  "apps/k7-daw-wasm/song/Cargo.lock",
];

/** The files outside a crate's src/ that a crate compiles in with `include_str!`, from the repo root. */
export const INCLUDED = [
  "apps/k7-daw-frontend/packages/song-schema/src/builtinParams.generated.json",
  "apps/k7-daw-wasm/verbs/commands.json",
];

/** One hash over every crate's Cargo.toml and src/, what they compile in, and the two locks; `root` may be a copy of the repo's crates. */
export function rustSourceHash(root = ROOT) {
  const all = CRATES.flatMap((crate) => [
    join(root, crate, "Cargo.toml"),
    ...files(join(root, crate, "src")),
  ]);
  all.push(...INCLUDED.map((f) => join(root, f)));
  all.push(...LOCKS.map((l) => join(root, l)));
  return hashOf(all, root);
}

/** One hash over src/types/ and src/consts.generated.ts, as the bindings binary and biome left them. */
export function typesHash() {
  const src = join(PKG, "src");
  return hashOf(
    [...files(join(src, "types")), join(src, "consts.generated.ts")],
    src,
  );
}

/** The `path` dependencies of the facade's manifest, as crate folders from the repo root. */
export function pathDeps() {
  const facade = join(ROOT, CRATES[0]);
  const manifest = readFileSync(join(facade, "Cargo.toml"), "utf8");
  // A dependency is an inline table; [lib] and [[bin]] name a `path` too, as a plain key.
  return [...manifest.matchAll(/\{[^}]*\bpath\s*=\s*"([^"]+)"/g)]
    .map((m) => m[1])
    .map((p) => relative(ROOT, resolve(facade, p)).replaceAll("\\", "/"));
}
