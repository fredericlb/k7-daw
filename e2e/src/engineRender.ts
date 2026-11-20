// engineRender.ts — an engine document rendered offline by the command-line engine, and its peak read back.
// `--render` bounces the whole edit; `--decode` hands the frames back, so no WAV parser lands in the suite.
// The one thing a reader gets wrong: a missing binary FAILS with a sentence, it never skips (apps/k7-daw-frontend/packages/song/docs/rustCompilerHost.md, 908).
import { execFile } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { REPO } from "./projectTree.ts";

/** A mix at or above this peak (-60 dBFS) sounds (decision 903). */
export const AUDIBLE = 0.001;

/** One render's bound: the longest project takes about six seconds. */
const RENDER_MS = 60_000;

/** What a render left: its exit code, what it said, whether it wrote a mix, and that mix's peak. */
export type Rendered = {
  code: number;
  said: string;
  mix: boolean;
  peak: number | null;
  seconds: number;
};

/** The engine binary: `path`, else CONFORMANCE_ENGINE_BIN, else the repo's `dist/k7-runtime`; a missing one throws. */
export function engineBinary(path?: string): string {
  const bin =
    path ??
    process.env.CONFORMANCE_ENGINE_BIN ??
    join(
      REPO,
      "dist",
      "k7-runtime",
      process.platform === "win32" ? "k7-runtime.exe" : "k7-runtime",
    );
  if (!existsSync(bin) || !statSync(bin).isFile())
    throw new Error(
      `engine binary not found at '${bin}': build it with 'node apps/k7-daw-engine/scripts/build-engine.mjs' or set CONFORMANCE_ENGINE_BIN`,
    );
  return bin;
}

type Ran = { code: number; out: Buffer; err: string };

let settings: string | undefined;

/** A scratch settings folder for every engine run here, so none writes the user's Settings.xml (engine docs/settingsDir.md). */
function settingsDir(): string {
  if (process.env.K7_SETTINGS_DIR) return process.env.K7_SETTINGS_DIR;
  if (!settings) {
    settings = mkdtempSync(join(tmpdir(), "k7-settings-"));
    process.on("exit", () =>
      rmSync(settings as string, { recursive: true, force: true }),
    );
  }
  return settings;
}

const run = (bin: string, args: string[]): Promise<Ran> =>
  new Promise((done) => {
    execFile(
      bin,
      args,
      {
        timeout: RENDER_MS,
        maxBuffer: 1 << 30,
        encoding: "buffer",
        env: { ...process.env, K7_SETTINGS_DIR: settingsDir() },
      },
      (error, out, err) => {
        const code = (error as { code?: number | string } | null)?.code;
        done({
          code: error ? (typeof code === "number" ? code : -1) : 0,
          out,
          err: err.toString("utf8"),
        });
      },
    );
  });

/** The highest absolute sample of a WAV, from `--decode`: one JSON line, then interleaved float32 little-endian. */
export async function peakOf(
  wav: string,
  bin = engineBinary(),
): Promise<number> {
  const r = await run(bin, ["--decode", wav]);
  if (r.code !== 0)
    throw new Error(`--decode ${wav} exited ${r.code}: ${r.err}`);
  const nl = r.out.indexOf(0x0a);
  const head = JSON.parse(r.out.subarray(0, nl).toString("utf8")) as {
    frames: number;
    channels: number;
  };
  const samples = head.frames * head.channels;
  const body = r.out.subarray(nl + 1);
  if (body.length !== samples * 4)
    throw new Error(
      `--decode ${wav}: ${body.length} bytes for ${samples} samples`,
    );
  let peak = 0;
  for (let i = 0; i < samples; i++)
    peak = Math.max(peak, Math.abs(body.readFloatLE(i * 4)));
  return peak;
}

/** The whole edit of `doc` rendered; a render that failed is reported as it ended, never thrown. */
export async function renderDoc(
  doc: unknown,
  bin = engineBinary(),
): Promise<Rendered> {
  const dir = mkdtempSync(join(tmpdir(), "k7-render-"));
  try {
    writeFileSync(join(dir, "doc.json"), JSON.stringify(doc));
    const t0 = Date.now();
    const r = await run(bin, [
      "--render",
      join(dir, "doc.json"),
      "--out",
      join(dir, "out"),
    ]);
    const seconds = (Date.now() - t0) / 1000;
    const mixPath = join(dir, "out", "mix.wav");
    const mix = r.code === 0 && existsSync(mixPath);
    return {
      code: r.code,
      said: r.err.trim(),
      mix,
      peak: mix ? await peakOf(mixPath, bin) : null,
      seconds,
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** True when a render left a mix that is audible. */
export const sounds = (r: Rendered): boolean =>
  r.peak !== null && r.peak >= AUDIBLE;
