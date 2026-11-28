// bindingsSources.mjs — the packages the bindings binary writes into, and the hash that tells one of them stale.
// A reader would get wrong what the hash covers: the Rust sources of the types and of the binary, not the output of a build.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(import.meta.dirname, "..", "..", "..");
export const PACKAGES = join(ROOT, "apps", "k7-daw-frontend", "packages");

/** The folders under packages/ the binary writes `generated/` into (`src/bindings.rs`'s list). */
export const TARGETS = ["engine-protocol", "graph-model", "web-protocol"];

/** The crates whose structs derive the types, and the binary that exports them, from the repo root. */
const INPUTS = [
  "apps/k7-daw-wasm/song/Cargo.toml",
  "apps/k7-daw-wasm/song/src",
  "apps/k7-daw-wasm/song-schema/Cargo.toml",
  "apps/k7-daw-wasm/song-schema/src",
  "apps/k7-daw-backend/packages/k7-file-service/src/wire",
  "apps/k7-daw-backend/packages/k7-front/src/routes/cameras/wire.rs",
  "apps/k7-daw-backend/packages/k7-front/src/routes/lan/peers.rs",
  "apps/k7-daw-backend/packages/k7-front/src/bindings.rs",
  "apps/k7-daw-backend/packages/k7-runtime-server/Cargo.toml",
  "apps/k7-daw-backend/packages/k7-runtime-server/src/wire",
  "apps/k7-daw-backend/packages/k7-runtime-server/src/server/state_frame.rs",
  "apps/k7-daw-backend/packages/k7-runtime-server/src/server/scan_board.rs",
  "apps/k7-daw-backend/packages/k7-runtime-server/src/engine/plugins.rs",
  "apps/k7-daw-backend/packages/k7-supervisor/src/protocol/wire.rs",
];

function files(path) {
  if (!statSync(path).isDirectory()) return [path];
  return readdirSync(path)
    .sort()
    .flatMap((name) => files(join(path, name)));
}

/** One hash over every input, line endings folded. */
export function bindingsHash() {
  const hash = createHash("sha256");
  for (const f of INPUTS.flatMap((p) => files(join(ROOT, p)))) {
    const rel = relative(ROOT, f).replaceAll("\\", "/");
    hash.update(
      `${rel}\0${readFileSync(f, "utf8").replaceAll("\r\n", "\n")}\0`,
    );
  }
  return hash.digest("hex").slice(0, 16);
}

/** The hash a package's generated index carries, if it does. */
export function stampOf(target) {
  const text = readFileSync(
    join(PACKAGES, target, "generated", "index.ts"),
    "utf8",
  );
  return text.match(/^\/\/ SOURCES ([0-9a-f]+)$/m)?.[1];
}
