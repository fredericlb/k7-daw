// onnxRuntime.mjs — ONNX Runtime for the supervisor's stem separation, from Microsoft's NuGet packages, hashes pinned:
// the shared library a computer's supervisor LOADS from beside itself, and the static xcframework iOS links.
// What a reader gets wrong: the packages are cached once per machine (`~/.cache/k7-onnxruntime`), never in a checkout. See docs/buildEngine.md.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { windowsTar } from "./buildTools.mjs";

const NUGET = "https://api.nuget.org/v3-flatcontainer";
/** Every package used, by name: Windows takes the DirectML build, which stops at 1.24, and DirectML itself. */
const PACKAGES = {
  ort: {
    url: `${NUGET}/microsoft.ml.onnxruntime/1.30.0/microsoft.ml.onnxruntime.1.30.0.nupkg`,
    sha256: "30df119bd9164eaa187666d93147448fdd4a9a1a3caf83f1505daf7af2d1c5d2",
  },
  ortDirectml: {
    url: `${NUGET}/microsoft.ml.onnxruntime.directml/1.24.4/microsoft.ml.onnxruntime.directml.1.24.4.nupkg`,
    sha256: "57e9f11b73437bef7a309496135d4c1f96b1a8e9ddba60013fa27bfc1d788681",
  },
  directml: {
    url: `${NUGET}/microsoft.ai.directml/1.15.4/microsoft.ai.directml.1.15.4.nupkg`,
    sha256: "4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9",
  },
};

const cacheDir = () =>
  process.env.K7_ORT_CACHE || join(homedir(), ".cache", "k7-onnxruntime");

/** The package's archive, downloaded once and checked at every use; a digest that does not match is refused. */
async function archive(name) {
  const pin = PACKAGES[name];
  const file = join(cacheDir(), pin.url.split("/").pop());
  if (!existsSync(file)) {
    mkdirSync(cacheDir(), { recursive: true });
    console.log(`▸ fetching ${pin.url}`);
    const res = await fetch(pin.url);
    if (!res.ok) throw new Error(`${pin.url}: ${res.status}`);
    writeFileSync(`${file}.part`, Buffer.from(await res.arrayBuffer()));
    renameSync(`${file}.part`, file);
  }
  const sum = createHash("sha256").update(readFileSync(file)).digest("hex");
  if (sum !== pin.sha256) {
    rmSync(file);
    throw new Error(`${name}: SHA-256 ${sum}, pinned ${pin.sha256} — refused`);
  }
  return file;
}

/** Unpack `members` (folders inside a zip) into `out`: bsdtar reads a zip on macOS and Windows, unzip elsewhere. */
function unzip(zip, out, members) {
  mkdirSync(out, { recursive: true });
  const [cmd, args] =
    process.platform === "linux"
      ? ["unzip", ["-q", "-o", zip, ...members.map((m) => `${m}/*`), "-d", out]]
      : [
          process.platform === "win32" ? windowsTar() : "tar",
          ["-xf", zip, "-C", out, ...members],
        ];
  const r = spawnSync(cmd, args, { stdio: "inherit" });
  if (r.status !== 0) throw new Error(`${cmd} could not unpack ${zip}`);
}

/** `member` of a package, unpacked once under the cache. */
async function unpacked(name, member) {
  const out = join(cacheDir(), `${name}-${PACKAGES[name].sha256.slice(0, 12)}`);
  if (!existsSync(join(out, member))) unzip(await archive(name), out, [member]);
  return join(out, member);
}

/** The library files a computer's supervisor loads, for this host; empty where ONNX Runtime ships none (Android, Intel Macs). */
export async function ortRuntime() {
  const { platform, arch } = process;
  const files = (dir, re) =>
    readdirSync(dir)
      .filter((f) => re.test(f))
      .map((f) => join(dir, f));
  if (platform === "win32" && arch === "x64") {
    const ort = await unpacked("ortDirectml", "runtimes/win-x64/native");
    const dml = await unpacked("directml", "bin/x64-win");
    return [
      join(ort, "onnxruntime.dll"),
      join(ort, "onnxruntime_providers_shared.dll"),
      join(dml, "DirectML.dll"),
    ];
  }
  const rid = {
    "darwin-arm64": "osx-arm64",
    "linux-x64": "linux-x64",
    "linux-arm64": "linux-arm64",
  }[`${platform}-${arch}`];
  if (!rid) return [];
  const dir = await unpacked("ort", `runtimes/${rid}/native`);
  return files(dir, /^libonnxruntime.*\.(dylib|so.*)$/);
}

/** The static onnxruntime.xcframework iOS links (ort-sys reads it from `ORT_IOS_XCFWK_PATH`). */
export async function ortIosXcframework() {
  const out = join(
    cacheDir(),
    `ios-xcframework-${PACKAGES.ort.sha256.slice(0, 12)}`,
  );
  const xcf = join(out, "onnxruntime.xcframework");
  if (!existsSync(join(xcf, "Info.plist"))) {
    const zip = join(
      await unpacked("ort", "runtimes/ios/native"),
      "onnxruntime.xcframework.zip",
    );
    rmSync(out, { recursive: true, force: true });
    unzip(zip, out, ["onnxruntime.xcframework"]);
  }
  return xcf;
}
