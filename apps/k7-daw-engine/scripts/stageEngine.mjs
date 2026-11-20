// stageEngine.mjs — copy the newest engine and supervisor builds into an app's staging `bin/`, with what they load beside them.
// The supervisor finds the engine beside itself, and the supervisor its onnxruntime.

import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  statSync,
} from "node:fs";
import { dirname, join } from "node:path";

const exe = (n) => (process.platform === "win32" ? `${n}.exe` : n);
/** The presets that build for a phone, whose binaries this machine cannot run. */
export const CROSS = /^(android|ios)-/;

/** The newest build of a CMake target under engine/<project>/build/<preset>/[<config>/], Debug last, or null. */
export function findBuild(repo, project, name) {
  const build = join(repo, "apps", "k7-daw-engine", project, "build");
  const found = [];
  if (existsSync(build))
    // A multi-config generator (Visual Studio) puts the binary one level down; a phone's preset is never staged.
    for (const preset of readdirSync(build).filter((p) => !CROSS.test(p)))
      for (const cfg of ["", "Release", "RelWithDebInfo", "Debug"]) {
        const path = join(build, preset, cfg, exe(name));
        if (existsSync(path) && statSync(path).isFile())
          found.push({ path, debug: cfg === "Debug" });
      }
  found.sort(
    (a, b) =>
      Number(a.debug) - Number(b.debug) ||
      statSync(b.path).mtimeMs - statSync(a.path).mtimeMs,
  );
  return found[0]?.path ?? null;
}

/** Like `findBuild`, but a missing build is an error: staging cannot go on without it. */
function newestBuild(repo, project, name) {
  const path = findBuild(repo, project, name);
  if (!path)
    throw new Error(
      `no build of ${name} under ${join(repo, "apps", "k7-daw-engine", project, "build")} — build it first`,
    );
  return path;
}

/** Fill `bin` (created) from the checkout at `repo`, or from `$STUDIO_PREBUILT_BIN`, a bin/ staged elsewhere (CI's engines job). */
export function stageEngine({ repo, bin }) {
  mkdirSync(bin, { recursive: true });
  const prebuilt = process.env.STUDIO_PREBUILT_BIN;
  if (prebuilt) {
    if (!existsSync(join(prebuilt, exe("k7-supervisor"))))
      throw new Error(`no k7-supervisor in STUDIO_PREBUILT_BIN=${prebuilt}`);
    cpSync(prebuilt, bin, { recursive: true, verbatimSymlinks: true });
    return;
  }
  const supervisor = newestBuild(repo, "k7-supervisor", "k7-supervisor");
  copyFileSync(
    newestBuild(repo, "k7-runtime", "k7-runtime"),
    join(bin, exe("k7-runtime")),
  );
  copyFileSync(supervisor, join(bin, exe("k7-supervisor")));
  const ort = readdirSync(dirname(supervisor)).filter((f) =>
    /^(libonnxruntime.*\.(dylib|so.*)|onnxruntime.*\.dll|DirectML\.dll)$/.test(
      f,
    ),
  );
  // Not fatal: the app runs, and says it cannot cut stems.
  if (!ort.length)
    console.warn(
      "warning: no ONNX Runtime beside the supervisor — the packaged app will not separate stems",
    );
  for (const f of ort)
    cpSync(join(dirname(supervisor), f), join(bin, f), {
      verbatimSymlinks: true,
    });
}
