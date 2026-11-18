// makeTranscodeVectors.ts — writes transcode-vectors.json: cuts and joins as node's `transcode()` writes them, by
// SHA-256; k7-mediatek's tests/all/transcode.rs holds Rust to them.
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type Piece, transcode } from "@studio/audio-analysis/transcode";

/** The loudness a normalised cut is brought to. */
const CLIP_LOUDNESS = { lufs: -14, truePeak: -1 };

/** A 16-bit PCM WAV: a tone per channel under a fade, and a little seeded noise. */
function wav(rate: number, channels: number, seconds: number): Buffer {
  const frames = Math.round(rate * seconds);
  const b = Buffer.alloc(44 + frames * channels * 2);
  b.write("RIFF", 0, "ascii");
  b.writeUInt32LE(36 + frames * channels * 2, 4);
  b.write("WAVEfmt ", 8, "ascii");
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(channels, 22);
  b.writeUInt32LE(rate, 24);
  b.writeUInt32LE(rate * channels * 2, 28);
  b.writeUInt16LE(channels * 2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36, "ascii");
  b.writeUInt32LE(frames * channels * 2, 40);
  let s = rate + channels;
  for (let i = 0; i < frames; i++)
    for (let c = 0; c < channels; c++) {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      const tone = Math.sin((2 * Math.PI * (196 + 98 * c) * i) / rate);
      const v =
        0.3 * tone * Math.sin((Math.PI * i) / frames) +
        (s / 0x7fffffff - 0.5) * 0.01;
      b.writeInt16LE(Math.round(v * 32767), 44 + (i * channels + c) * 2);
    }
  return b;
}

/** A case: its pieces by source name, the channels asked, and whether the clip loudness is. */
interface Case {
  name: string;
  pieces: (Omit<Piece, "source"> & { source: string })[];
  channels: 0 | 1 | 2;
  loudness: boolean;
}

// Every path of transcode.ts a recorder takes: a seek in WebM, WAV and FLAC, a cut under the pre-roll, silence, joins.
const CASES: Case[] = [
  {
    name: "clip-webm",
    pieces: [{ source: "take.webm", start: 1.2, duration: 1.1 }],
    channels: 0,
    loudness: true,
  },
  {
    name: "clip-wav",
    pieces: [{ source: "phone.wav", start: 0.6, duration: 0.8 }],
    channels: 0,
    loudness: true,
  },
  {
    name: "clip-flac",
    pieces: [{ source: "tone.flac", start: 0.7, duration: 0.9 }],
    channels: 0,
    loudness: true,
  },
  {
    name: "clip-head",
    pieces: [{ source: "phone.wav", start: 0.1, duration: 0.5 }],
    channels: 0,
    loudness: true,
  },
  {
    name: "clip-silence",
    pieces: [{ source: "silence.flac", start: 120, duration: 1 }],
    channels: 0,
    loudness: true,
  },
  {
    name: "join",
    pieces: [{ source: "take.webm" }, { source: "phone.wav" }],
    channels: 2,
    loudness: false,
  },
  {
    name: "join-three",
    pieces: [
      { source: "phone.wav" },
      { source: "tone.flac" },
      { source: "take.webm" },
    ],
    channels: 2,
    loudness: false,
  },
  {
    name: "plain-wav",
    pieces: [{ source: "phone.wav" }],
    channels: 0,
    loudness: false,
  },
  {
    name: "plain-flac",
    pieces: [{ source: "tone.flac" }],
    channels: 0,
    loudness: false,
  },
];

const audio = join(
  import.meta.dirname,
  "../../../../e2e/specs/conformance/audio",
);
const [out = join(import.meta.dirname, "transcode-vectors.json")] =
  process.argv.slice(2);

async function transcodeVectors(): Promise<void> {
  const work = mkdtempSync(join(tmpdir(), "k7-transcode-vectors-"));
  try {
    writeFileSync(join(work, "take.wav"), wav(48_000, 2, 3));
    await transcode(
      [{ source: join(work, "take.wav") }],
      join(work, "take.webm"),
      {
        kind: "opus",
        bitrate: 128_000,
      },
    );
    const sources: Record<string, Buffer> = {
      "take.webm": readFileSync(join(work, "take.webm")),
      "phone.wav": wav(16_000, 1, 2),
      "tone.flac": readFileSync(join(audio, "tone.flac")),
      "silence.flac": readFileSync(join(audio, "silence.flac")),
    };
    for (const [name, bytes] of Object.entries(sources))
      writeFileSync(join(work, name), bytes);
    const cases = [];
    for (const c of CASES) {
      const dest = join(work, `${c.name}.webm`);
      const pieces = c.pieces.map((p) => ({
        ...p,
        source: join(work, p.source),
      }));
      const { frames, gainDb } = await transcode(
        pieces,
        dest,
        { kind: "opus", bitrate: 128_000, channels: c.channels || undefined },
        c.loudness ? { loudness: CLIP_LOUDNESS } : {},
      );
      const webm = readFileSync(dest);
      cases.push({
        ...c,
        frames,
        gainDb,
        bytes: webm.length,
        sha256: createHash("sha256").update(webm).digest("hex"),
      });
    }
    // The conformance suite's FLACs are named, not copied: the tests read them where they lie.
    const embedded = Object.fromEntries(
      ["take.webm", "phone.wav"].map((n) => [n, sources[n].toString("base64")]),
    );
    writeFileSync(
      out,
      `${JSON.stringify({ sources: embedded, conformance: ["tone.flac", "silence.flac"], cases }, null, 1)}\n`,
    );
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

void transcodeVectors();
