// makeHostVectors.mts — records the TypeScript's own answers for the host members the Rust front ports (3f): ffprobe's arguments
// and its JSON read into facts. The Rust replays them in `cargo test`; a drift fails check-vectors.sh. Run with an absolute path.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  CLIS,
  extractAudioArgs,
  parseSrt,
  srtOf,
} from "../../k7-plugins/src/transcription/transcribe.ts";
import { copyPathOf } from "../../video-prep/src/copyPath.ts";
import { factsOf, probeArgs } from "../../video-prep/src/probe.ts";
import { copyArgs, PROXY_HEIGHT } from "../../video-prep/src/transcode.ts";

const here = dirname(fileURLToPath(import.meta.url));
const vectorsDir = join(
  here,
  "../../../apps/k7/backend-rs/crates/k7-mediatek/tests/vectors",
);
const out = join(vectorsDir, "host-probe.json");

const sources = [
  "/w/a.mp4",
  "/w/with space/é.mov",
  "a:b.mp4",
  "pipe:0",
  "-",
  "file:/x/y.mkv",
  "C:\\films\\a.mp4",
];

const stream = (extra: Record<string, unknown> = {}) => ({
  codec_name: "h264",
  width: 1920,
  height: 1080,
  pix_fmt: "yuv420p",
  avg_frame_rate: "30000/1001",
  ...extra,
});
const format = {
  duration: "12.500000",
  format_name: "mov,mp4,m4a,3gp,3g2,mj2",
};
const doc = (s: unknown, f: unknown = format) =>
  JSON.stringify({ streams: s === undefined ? undefined : [s], format: f });

const cases: Record<string, string> = {
  typical: doc(stream()),
  "no stream": JSON.stringify({ streams: [], format }),
  "no streams key": JSON.stringify({ format }),
  "no format": doc(stream(), undefined),
  "rotation -90": doc(stream({ side_data_list: [{ rotation: -90 }] })),
  "rotation 90": doc(stream({ side_data_list: [{ rotation: 90 }] })),
  "rotation 180": doc(stream({ side_data_list: [{ rotation: 180 }] })),
  "rotation 450": doc(stream({ side_data_list: [{ rotation: 450 }] })),
  "rotation 44 rounds to none": doc(
    stream({ side_data_list: [{ rotation: 44 }] }),
  ),
  "rotation 46": doc(stream({ side_data_list: [{ rotation: 46 }] })),
  "rotation -45 is none": doc(stream({ side_data_list: [{ rotation: -45 }] })),
  "rotation 135": doc(stream({ side_data_list: [{ rotation: 135 }] })),
  "rotation -135": doc(stream({ side_data_list: [{ rotation: -135 }] })),
  "rotate tag": doc(stream({ tags: { rotate: "90" } })),
  "side data without rotation first": doc(
    stream({ side_data_list: [{ type: "Display Matrix" }, { rotation: 270 }] }),
  ),
  "null rotation falls to the tag": doc(
    stream({ side_data_list: [{ rotation: null }], tags: { rotate: "180" } }),
  ),
  "side data wins over the tag": doc(
    stream({ side_data_list: [{ rotation: 90 }], tags: { rotate: "180" } }),
  ),
  "rate 25": doc(stream({ avg_frame_rate: "25" })),
  "rate 0/0": doc(stream({ avg_frame_rate: "0/0" })),
  "rate 24/1": doc(stream({ avg_frame_rate: "24/1" })),
  "rate missing": doc(stream({ avg_frame_rate: undefined })),
  "rate junk": doc(stream({ avg_frame_rate: "abc" })),
  "sizes as strings": doc(stream({ width: "1280", height: "720" })),
  "duration missing": doc(stream(), { format_name: "matroska,webm" }),
  "duration junk": doc(stream(), { duration: "N/A", format_name: "avi" }),
  "names missing": doc({ width: 8, height: 8 }),
};

const vectors = {
  probeArgs: sources.map((source) => ({ source, args: probeArgs(source) })),
  facts: Object.entries(cases).map(([name, json]) => ({
    name,
    json,
    facts: factsOf(json),
  })),
};
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(vectors, null, 2)}\n`);

// The copies prepare makes: ffmpeg's arguments per kind and where each lands.
const copySources = [
  "/w/a.mp4",
  "media/a b.mov",
  "x/y.z/film",
  "film",
  ".mp4",
  "a.b.mkv",
  "d.v/e.f.webm",
  "é.MOV",
  "a/",
];
const kinds = ["remux", "transcode", "proxy"] as const;
const copies = {
  proxyHeight: PROXY_HEIGHT,
  args: copySources.flatMap((source) =>
    kinds.map((kind) => ({
      kind,
      source,
      dest: `${source}.part`,
      args: copyArgs(kind, source, `${source}.part`),
    })),
  ),
  paths: copySources.flatMap((source) =>
    (["optimized", "proxy"] as const).map((kind) => ({
      source,
      kind,
      path: copyPathOf(source, kind),
    })),
  ),
};
writeFileSync(
  join(vectorsDir, "host-copy.json"),
  `${JSON.stringify(copies, null, 2)}\n`,
);

// Whisper's half: both CLIs' arguments and output files, the audio extract's arguments, the SRT read and written.
const whisperErr = (f: () => unknown): string | null => {
  try {
    f();
    return null;
  } catch (e) {
    return (e as Error).message;
  }
};
const srtTexts: Record<string, string> = {
  openai:
    "1\n00:00:01,000 --> 00:00:02,500\nHello there\n\n2\n00:00:03,000 --> 00:00:04,000\nSecond line\nand a third\n",
  "whisper.cpp dots":
    "1\n00:00:00.480 --> 00:00:01.120\n Bonjour\n\n2\n00:01:02.5 --> 01:00:00.05\nÇa va\n",
  crlf: "1\r\n00:00:01,000 --> 00:00:02,000\r\nA\r\n\r\n2\r\n00:00:02,000 --> 00:00:03,000\r\nB\r\n",
  "no index": "00:00:01,000 --> 00:00:02,000\nNo index here\n",
  "garbled index": "x\ny\n00:00:01,000 --> 00:00:02,000\ntext after\n",
  "empty cue dropped":
    "1\n00:00:01,000 --> 00:00:02,000\n\n\n2\n00:00:02,000 --> 00:00:03,000\nkept\n",
  "three blank lines":
    "1\n00:00:01,000 --> 00:00:02,000\nA\n\n\n\n2\n00:00:02,000 --> 00:00:03,000\nB\n",
  "no timing": "just words\n\nmore words\n",
  "spaced arrow": "1\n00:00:01,000   -->   00:00:02,000\nA\n",
  "long hours": "1\n100:00:01,000 --> 100:00:02,000\nA\n",
  empty: "",
};
const segmentCases: Record<
  string,
  { start: number; end: number; text: string }[]
> = {
  one: [{ start: 1, end: 2.5, text: "Hello" }],
  many: [
    { start: 0, end: 0.4, text: "a" },
    { start: 3661.001, end: 3662.0005, text: "b" },
  ],
  negative: [{ start: -1, end: 0.0004, text: "c" }],
  rounding: [{ start: 1.0005, end: 2.0015, text: "d" }],
  none: [],
};
const whisper = {
  order: Object.keys(CLIS),
  cli: Object.entries(CLIS).flatMap(([bin, cli]) =>
    ["/tmp/k7 x/audio.wav", "/t/audio.wav", "/t/clip.WAV"].flatMap((wav) =>
      [undefined, "base", "/m/ggml-base.bin"].map((model) => ({
        bin,
        wav,
        outDir: "/t",
        model: model ?? null,
        args: (() => {
          try {
            return cli.args(wav, "/t", model);
          } catch {
            return null;
          }
        })(),
        error: whisperErr(() => cli.args(wav, "/t", model)),
        srt: cli.srt(wav, "/t"),
      })),
    ),
  ),
  extractAudio: ["/w/a.mp4", "a:b.mp4", "pipe:0"].map((video) => ({
    video,
    wav: "/t/audio.wav",
    args: extractAudioArgs(video, "/t/audio.wav"),
  })),
  parse: Object.entries(srtTexts).map(([name, text]) => ({
    name,
    text,
    segments: parseSrt(text),
  })),
  write: Object.entries(segmentCases).map(([name, segments]) => ({
    name,
    segments,
    srt: srtOf(segments),
  })),
};
writeFileSync(
  join(vectorsDir, "host-whisper.json"),
  `${JSON.stringify(whisper, null, 2)}\n`,
);
