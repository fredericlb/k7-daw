// stage.mjs — everything the Xcode project takes from outside it, laid out under build/staged: the engine as an
// xcframework, the Rust front and supervisor as one static library with ONNX Runtime's static xcframework beside it, the CA bundle as pinned in pins.json (checked by its
// SHA-256), the shell page as `cap copy` would lay it out, and what the front reads. Run BEFORE
// Xcode: it plans an xcframework's slices when the build starts, so one staged mid-build is not seen.
// Usage: node apps/k7-daw-ios/scripts/stage.mjs [--sim] [--only=engine,embed,cacert,shell,backend]
// See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ortIosXcframework } from "../../k7-daw-engine/scripts/onnxRuntime.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const repo = join(app, "..", "..");
const staged = join(app, "build", "staged");
const frameworks = join(staged, "frameworks");
const resources = join(staged, "resources");
const cache = join(app, "build", "cache");
const pins = JSON.parse(readFileSync(join(here, "pins.json"), "utf8"));

const sim = process.argv.includes("--sim");
const preset = sim ? "ios-sim-arm64" : "ios-arm64";
const onlyArg = process.argv.find((a) => a.startsWith("--only="));
const only = onlyArg
  ? new Set(onlyArg.slice("--only=".length).split(","))
  : null;
const wanted = (part) => !only || only.has(part);

function run(cmd, args, cwd = repo, env = {}) {
  console.log(`[stage] ${cmd} ${args.join(" ")}`);
  execFileSync(cmd, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
}

/** `framework` as the one slice of an xcframework at `out`, which is what the project names. */
function xcframework(framework, out) {
  if (!existsSync(framework))
    throw new Error(`no ${framework}: its build did not produce it`);
  rmSync(out, { recursive: true, force: true });
  run("xcodebuild", [
    "-create-xcframework",
    "-framework",
    framework,
    "-output",
    out,
  ]);
}

/** A pinned download, from the cache when it is already there; a digest that does not match fails the stage. */
async function pinned(name) {
  const pin = pins[name];
  const file = join(cache, pin.url.split("/").pop());
  if (!existsSync(file)) {
    mkdirSync(cache, { recursive: true });
    console.log(`[stage] fetching ${pin.url}`);
    const res = await fetch(pin.url);
    if (!res.ok) throw new Error(`${pin.url}: ${res.status}`);
    writeFileSync(file, Buffer.from(await res.arrayBuffer()));
  }
  const sum = createHash("sha256").update(readFileSync(file)).digest("hex");
  if (sum !== pin.sha256) {
    rmSync(file);
    throw new Error(`${name}: SHA-256 ${sum}, pinned ${pin.sha256} — refused`);
  }
  return file;
}

mkdirSync(frameworks, { recursive: true });
mkdirSync(resources, { recursive: true });

if (wanted("engine")) {
  run("node", ["apps/k7-daw-engine/k7-runtime/scripts/build.mjs", preset]);
  xcframework(
    join(
      repo,
      "apps/k7-daw-engine/k7-runtime/build",
      preset,
      "k7runtime.framework",
    ),
    join(frameworks, "k7runtime.xcframework"),
  );
}

// The Rust front and the supervisor, linked into the app: k7-embed as a static library, since a cdylib for iOS needs Apple's own linker.
// ONNX Runtime's static xcframework is linked by the project beside it, and named to ort-sys for the build.
if (wanted("embed")) {
  const ort = join(frameworks, "onnxruntime.xcframework");
  rmSync(ort, { recursive: true, force: true });
  cpSync(await ortIosXcframework(), ort, {
    recursive: true,
    verbatimSymlinks: true,
  });
  const target = sim ? "aarch64-apple-ios-sim" : "aarch64-apple-ios";
  const targetDir = join(app, "build", "cargo");
  run(
    "cargo",
    [
      "rustc",
      "--release",
      "-p",
      "k7-embed",
      "--target",
      target,
      "--crate-type",
      "staticlib",
      "--target-dir",
      targetDir,
    ],
    join(repo, "apps/k7-daw-backend"),
    { IPHONEOS_DEPLOYMENT_TARGET: "17.0", ORT_IOS_XCFWK_PATH: ort },
  );
  mkdirSync(join(staged, "lib"), { recursive: true });
  cpSync(
    join(targetDir, target, "release/libk7_embed.a"),
    join(staged, "lib/libk7_embed.a"),
  );
}

if (wanted("cacert")) {
  cpSync(await pinned("cacert"), join(resources, "cacert.pem"));
}

// The shell page Android shows too, laid out as `cap copy` lays it: public/ beside the config Capacitor reads at start.
if (wanted("shell")) {
  const web = join(repo, "apps/k7-daw-android/web");
  run("npm", ["run", "build"], web);
  rmSync(join(resources, "public"), { recursive: true, force: true });
  cpSync(join(web, "dist"), join(resources, "public"), { recursive: true });
  // No npm plugin to load: the app's two are registered by MainViewController, and Capacitor wants the empty list said.
  const config = JSON.parse(
    readFileSync(join(web, "capacitor.config.json"), "utf8"),
  );
  writeFileSync(
    join(resources, "capacitor.config.json"),
    JSON.stringify({ ...config, packageClassList: [] }, null, 2),
  );
  // The three page scripts both apps inject, kept where Android keeps them.
  const scripts = join(resources, "page-scripts");
  mkdirSync(scripts, { recursive: true });
  for (const name of ["account.js", "media-session.js", "top-color.js"])
    cpSync(
      join(repo, "apps/k7-daw-android/app/src/main/assets", name),
      join(scripts, name),
    );
}

// What the front reads: the built page, the library it seeds, its code root and the desktop's table.
if (wanted("backend")) {
  run("npm", ["exec", "--workspace", "k7-frontend", "--", "vite", "build"]);
  const out = join(resources, "backend");
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  cpSync(join(repo, "apps/k7-daw-frontend/dist"), join(out, "dist"), {
    recursive: true,
  });
  const rust = join(repo, "apps/k7-daw-backend/scripts");
  run("node", [join(rust, "stageCodeRoot.mjs"), join(out, "code")]);
  run("node", [
    "--input-type=module",
    "-e",
    `const m = await import(${JSON.stringify(join(rust, "libraryArchive.mjs"))}); await m.bundleLibrary(${JSON.stringify(repo)}, ${JSON.stringify(join(out, "k7-core"))});`,
  ]);
  cpSync(
    join(repo, "apps/k7-daw-backend/standalone.json"),
    join(out, "standalone.json"),
  );
}

// The build number is the commit count, as Android's versionCode: App Store Connect refuses a number it has seen (docs/versioning.md).
{
  const git = (...args) =>
    execFileSync("git", args, { cwd: repo, encoding: "utf8" }).trim();
  if (git("rev-parse", "--is-shallow-repository") === "true")
    throw new Error(
      "a shallow clone has no history to count for the build number (docs/versioning.md)",
    );
  const count = git("rev-list", "--count", "HEAD");
  mkdirSync(staged, { recursive: true });
  writeFileSync(
    join(staged, "version.xcconfig"),
    `CURRENT_PROJECT_VERSION = ${count}\n`,
  );
  console.log(`[stage] build number ${count}`);
}

console.log(
  `[stage] staged for ${sim ? "the simulator" : "a device"} in ${staged}`,
);
