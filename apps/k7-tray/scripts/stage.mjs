// stage.mjs — lay out src-tauri/resources for `tauri build` (and `tauri dev`): k7's backend bundle with the frontend
// in it, standalone.json, the engine and the supervisor, and the node the bundle runs on, pinned and checksummed.
// The engine comes from the repo's builds, or `$STUDIO_PREBUILT_BIN` in CI (engine/scripts/docs/buildEngine.md).

import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { run, windowsTar } from "../../../engine/scripts/buildTools.mjs";
import { stageEngine } from "../../../engine/scripts/stageEngine.mjs";
import { stageCodeRoot } from "../../k7/backend-rs/scripts/stageCodeRoot.mjs";

/** The node the backend runs on: Node 24 LTS, as .nvmrc says; bumped by hand, its checksum read from nodejs.org. */
const NODE_VERSION = "24.22.0";

const app = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(app, "..", "..");
const out = join(app, "src-tauri", "resources");
const cache = join(repo, "dist", "cache", "node");

/** Node's archive for this machine, and the path of the binary inside it. */
function nodeArchive() {
  const arch = process.arch === "arm64" ? "arm64" : "x64";
  const base = `node-v${NODE_VERSION}`;
  if (process.platform === "win32")
    return {
      file: `${base}-win-${arch}.zip`,
      inner: `${base}-win-${arch}/node.exe`,
    };
  const os = process.platform === "darwin" ? "darwin" : "linux";
  return {
    file: `${base}-${os}-${arch}.tar.gz`,
    inner: `${base}-${os}-${arch}/bin/node`,
  };
}

/** A stalled download is cut at this, body included, and tried again: a CI runner otherwise hangs ten minutes then fails. */
const DOWNLOAD_MS = 120_000;
const DOWNLOAD_TRIES = 4;

async function download(url) {
  for (let attempt = 1; ; attempt++) {
    let status = 0;
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(DOWNLOAD_MS) });
      status = r.status;
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return Buffer.from(await r.arrayBuffer());
    } catch (e) {
      // A 4xx is an answer, not a hiccup: a wrong version would only be asked for again.
      const final = status >= 400 && status < 500;
      if (final || attempt >= DOWNLOAD_TRIES)
        throw new Error(`${url}: ${e.message}`);
      console.log(
        `  ${url}: ${e.message}, try ${attempt + 1} of ${DOWNLOAD_TRIES}`,
      );
      await new Promise((r) => setTimeout(r, 2000 * attempt));
    }
  }
}

/** The archive, from the cache or nodejs.org, refused unless its SHA-256 is the one SHASUMS256.txt names. */
async function nodeBinary() {
  const { file, inner } = nodeArchive();
  const dist = `https://nodejs.org/dist/v${NODE_VERSION}`;
  mkdirSync(cache, { recursive: true });
  const sums = (await download(`${dist}/SHASUMS256.txt`)).toString("utf8");
  const want = sums
    .split("\n")
    .find((l) => l.trim().endsWith(`  ${file}`))
    ?.split(/\s+/)[0];
  if (!want) throw new Error(`${file} is not in node's SHASUMS256.txt`);
  const archive = join(cache, file);
  if (!existsSync(archive))
    writeFileSync(archive, await download(`${dist}/${file}`));
  const got = createHash("sha256").update(readFileSync(archive)).digest("hex");
  if (got !== want) {
    rmSync(archive, { force: true });
    throw new Error(`${file}: sha256 ${got}, expected ${want}`);
  }
  const unpacked = join(cache, `v${NODE_VERSION}`);
  rmSync(unpacked, { recursive: true, force: true });
  mkdirSync(unpacked, { recursive: true });
  // Windows' bsdtar reads the .zip node ships there.
  const tar = process.platform === "win32" ? windowsTar() : "tar";
  run(tar, ["-x", "-f", archive, "-C", unpacked, inner], repo);
  return join(unpacked, inner);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

console.log("· frontend");
run("npm", ["exec", "--workspace", "k7-frontend", "--", "vite", "build"], repo);

console.log("· backend");
run(
  "node",
  [
    join(repo, "apps", "k7", "backend", "scripts", "bundle.mjs"),
    join(out, "backend"),
  ],
  repo,
);
copyFileSync(
  join(repo, "apps", "k7", "backend", "standalone.json"),
  join(out, "standalone.json"),
);

// The files the Rust front's actor reads at its code root, which the tray names to it as K7_CODE_ROOT (docs/tray.md).
const code = stageCodeRoot(join(out, "code"));
console.log(`· the front's code root, ${(code / 1024).toFixed(0)} KB`);

console.log("· engine and supervisor");
stageEngine({ repo, bin: join(out, "bin") });

console.log(`· node ${NODE_VERSION}`);
mkdirSync(join(out, "node"), { recursive: true });
const node = await nodeBinary();
const name = process.platform === "win32" ? "node.exe" : "node";
copyFileSync(node, join(out, "node", name));
chmodSync(join(out, "node", name), 0o755);

console.log(`staged in ${out}`);
