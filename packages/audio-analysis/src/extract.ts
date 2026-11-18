// extract.ts — a media file's first audio stream, written out as a 32-bit float WAV at its own
// rate and channel count. The WebAssembly decoder first, which reads a camera's mp4 or .mov and a
// browser's webm on every host; ffmpeg only for a file it cannot open; the engine's `--decode`
// last, when there is no ffmpeg either. Nothing is resampled or mixed down. See docs/analysis.md.

import { closeSync, openSync, renameSync, rmSync, writeSync } from "node:fs";
import {
  type DecodeOptions,
  decodeFrames,
  NoFfmpegError,
  type StreamFacts,
} from "./decode";
import { WasmOpenError } from "./decodeWasm";
import { floatFormat, wavHeader } from "./wav";

// A float format's header has the same length whatever the stream, so a blank one holds its place.
const HEADER = wavHeader(floatFormat(1, 1), 0).length;

// The RIFF header of a float WAV holding `frames` frames; written last, once the count is known.
const header = (f: StreamFacts): Buffer =>
  wavHeader(floatFormat(f.channels, f.sampleRate), f.frames * f.channels * 4);

// One decoder's attempt, into a temporary file renamed over `dest` only when it finished.
async function extractWith(
  source: string,
  dest: string,
  options: DecodeOptions,
): Promise<StreamFacts> {
  const tmp = `${dest}.part`;
  const fd = openSync(tmp, "w");
  let open = true;
  try {
    writeSync(fd, Buffer.alloc(HEADER));
    const facts = await decodeFrames(
      source,
      (samples, channels, frames) => {
        writeSync(
          fd,
          new Uint8Array(
            samples.buffer,
            samples.byteOffset,
            frames * channels * 4,
          ),
        );
      },
      options,
    );
    writeSync(fd, header(facts), 0, HEADER, 0);
    closeSync(fd);
    open = false;
    renameSync(tmp, dest);
    return facts;
  } catch (e) {
    if (open) closeSync(fd);
    rmSync(tmp, { force: true });
    throw e;
  }
}

/** Write `source`'s sound to `dest`; null when the file HAS no audio stream, which is a silent film rather than a fault. */
export async function extractSound(
  source: string,
  dest: string,
  { engineBin = null, signal }: DecodeOptions = {},
): Promise<StreamFacts | null> {
  try {
    // WebAssembly first on every host: the engine reads video containers on macOS and Windows only.
    return await extractWith(source, dest, {
      engineBin: null,
      wasm: true,
      signal,
    });
  } catch (e) {
    if (/no audio stream/.test((e as Error).message)) return null;
    // A file neither decoder here opened — WebAssembly could not, and there is no ffmpeg — goes to the engine.
    const unopened = e instanceof NoFfmpegError || e instanceof WasmOpenError;
    if (!unopened || !engineBin) throw e;
  }
  return extractWith(source, dest, { engineBin, signal });
}
