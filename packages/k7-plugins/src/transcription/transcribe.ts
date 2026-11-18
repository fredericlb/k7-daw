// transcribe.ts — `track transcribe`: whisper listens to the video a picture lane places, and the
// transcript lands as the NEW lane's `content:`. The CLI is whoever the machine has — `whisper`
// (openai-whisper) or `whisper-cli` (whisper.cpp, which needs STUDIO_WHISPER_MODEL to name a ggml
// model) — and timings are the SOURCE's own seconds, written as SRT so the text stays usable
// outside this app. See docs/verbs.md.

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { transcodeOffThread, WasmOpenError } from "@studio/audio-analysis/pool";
import { ffmpegBin, input, NoFfmpegError, run } from "@studio/ffmpeg";
import { parseSrt, srtOf, type TranscriptSegment } from "./srt";

export { parseSrt, srtOf, type TranscriptSegment };

const stderrTail = (err: Buffer | string | undefined): string =>
  String(err ?? "")
    .trim()
    .split("\n")
    .slice(-3)
    .join(" · ");

/** ffmpeg's arguments for the 16 kHz mono wav whisper reads, the user's video opened under the whitelist. */
export const extractAudioArgs = (video: string, wav: string): string[] => [
  "-y",
  "-hide_banner",
  "-loglevel",
  "error",
  ...input(video),
  "-vn", // no picture: it is the sound whisper listens to
  "-ac",
  "1",
  "-ar",
  "16000",
  wav,
];

/** Decoding the sound of an hour of video is seconds of work: past this, the decoder is stuck. */
const EXTRACT_TIMEOUT_MS = 10 * 60_000;

/** The audio whisper needs: decoded in WebAssembly, and by ffmpeg only for a container that cannot open. */
export async function extractAudio(video: string, wav: string): Promise<void> {
  try {
    await transcodeOffThread(
      [{ source: video }],
      wav,
      { kind: "wav", rate: 16_000, channels: 1 },
      { deadlineMs: EXTRACT_TIMEOUT_MS },
    );
    return;
  } catch (e) {
    if (!(e instanceof WasmOpenError)) throw e;
  }
  try {
    await run(ffmpegBin(), extractAudioArgs(video, wav), {
      timeoutMs: EXTRACT_TIMEOUT_MS,
      what: `reading ${basename(video)}`,
      why: "this video's container needs ffmpeg to be transcribed. Install it.",
    });
  } catch (e) {
    if (e instanceof NoFfmpegError) throw e;
    throw new Error(
      `ffmpeg could not read ${basename(video)}: ${(e as Error).message}`,
    );
  }
}

/** One whisper CLI's dialect: how it is invoked, and where its SRT lands. */
interface WhisperCli {
  args: (wav: string, outDir: string, model?: string) => string[];
  srt: (wav: string, outDir: string) => string;
}

export const CLIS: Record<string, WhisperCli> = {
  // openai-whisper names its output after the input's stem and manages its own model downloads.
  whisper: {
    args: (wav, outDir, model) => [
      wav,
      "--output_format",
      "srt",
      "--output_dir",
      outDir,
      ...(model ? ["--model", model] : []),
    ],
    srt: (wav, outDir) =>
      join(outDir, `${basename(wav).replace(/\.wav$/i, "")}.srt`),
  },
  // whisper.cpp wants the model spelled out — STUDIO_WHISPER_MODEL is a path to a ggml file.
  "whisper-cli": {
    args: (wav, outDir, model) => {
      if (!model)
        throw new Error(
          "whisper-cli needs a model — set STUDIO_WHISPER_MODEL to a ggml file (e.g. models/ggml-base.bin)",
        );
      return [
        "-f",
        wav,
        "-m",
        model,
        // Its default is English, which TRANSLATES any other speech; openai-whisper detects the language.
        "-l",
        "auto",
        "-osrt",
        "-of",
        join(outDir, "transcribe"),
      ];
    },
    srt: (_wav, outDir) => join(outDir, "transcribe.srt"),
  },
};

type Attempt =
  | { ok: true; srt: string }
  | { ok: false; missing: boolean; error?: string };

/** Whether `bin` is on this machine's PATH, asked by running its `--help`. */
const present = (bin: string): Promise<boolean> =>
  new Promise((res) => {
    const child = spawn(bin, ["--help"], { stdio: "ignore" });
    child.on("error", (e: NodeJS.ErrnoException) => res(e.code !== "ENOENT"));
    child.on("close", () => res(true));
  });

async function attempt(
  bin: string,
  cli: WhisperCli,
  wav: string,
  outDir: string,
): Promise<Attempt> {
  let args: string[];
  try {
    args = cli.args(wav, outDir, process.env.STUDIO_WHISPER_MODEL);
  } catch (e) {
    // A refusal the adapter itself raised, which only matters once the tool is there: else it is just missing.
    if (!(await present(bin))) return { ok: false, missing: true };
    return { ok: false, missing: false, error: (e as Error).message };
  }
  const srtPath = cli.srt(wav, outDir);
  return new Promise((res) => {
    const child = spawn(bin, args, { stdio: ["ignore", "ignore", "pipe"] });
    let err = "";
    child.stderr?.on("data", (d: Buffer) => {
      err = `${err}${String(d)}`.slice(-4000);
    });
    child.on("error", (e: NodeJS.ErrnoException) =>
      res(
        e.code === "ENOENT"
          ? { ok: false, missing: true }
          : { ok: false, missing: false, error: e.message },
      ),
    );
    child.on("close", (code) =>
      code === 0 && existsSync(srtPath)
        ? res({ ok: true, srt: srtPath })
        : res({
            ok: false,
            missing: false,
            error: stderrTail(err) || `exit code ${code ?? "?"}`,
          }),
    );
  });
}

/** Transcribe one 16 kHz mono wav with whoever the machine has, answering the segments.
 *  A binary that is not there is stepped over; one that RAN and failed IS the error, said as it
 *  said it — retrying with another dialect would double a model download nobody asked for. */
export async function transcribeAudio(
  wav: string,
  outDir: string,
): Promise<TranscriptSegment[]> {
  const notInstalled: string[] = [];
  for (const [bin, cli] of Object.entries(CLIS)) {
    const r = await attempt(bin, cli, wav, outDir);
    if (r.ok) return parseSrt(readFileSync(r.srt, "utf8"));
    if (r.missing) notInstalled.push(bin);
    else
      throw new Error(`${bin} failed: ${r.error ?? "it wrote no subtitles"}`);
  }
  throw new Error(
    `no whisper on this machine (tried ${notInstalled.join(", ")}) — install one: ` +
      `pip install openai-whisper gives \`whisper\`; whisper.cpp gives \`whisper-cli\``,
  );
}

/** The host member `transcribe`: whisper's SRT for `video`, or null when it heard no speech. */
export async function transcribeVideo(video: string): Promise<string | null> {
  const tmp = mkdtempSync(join(tmpdir(), "k7-transcribe-"));
  try {
    await extractAudio(video, join(tmp, "audio.wav"));
    const segments = await transcribeAudio(join(tmp, "audio.wav"), tmp);
    return segments.length ? srtOf(segments) : null;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}
