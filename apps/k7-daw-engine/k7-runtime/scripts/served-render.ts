// served-render.ts — the serving engine's `render` command against `--render`: the same mix sample for sample,
// two stems that differ, an mp3 source (the proxy timer), one render at a time, the roots, and the playing
// stream's realtime ratio and the message thread's pulse while renders run. Starts its own engine, silent.
//   npx tsx apps/k7-daw-engine/k7-runtime/scripts/served-render.ts        (docs/cliModes.md, "Rendered inside the engine")

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import WebSocket from "ws";
import { clipDoc, emptyDoc, pluginDoc, trackDoc } from "../../../k7-daw-frontend/packages/engine-protocol/build.ts";
import type { ProjectDoc } from "../../../k7-daw-frontend/packages/engine-protocol/project.ts";
import { findEngineBinary } from "../../../k7-daw-frontend/packages/engine-client/src/engineBinary.ts";
import { readWav, wavFile } from "./wav.ts";

const here = dirname(fileURLToPath(import.meta.url));
const bin = findEngineBinary();
if (!bin) throw new Error("no k7-runtime binary on this machine");

const scratch = mkdtempSync(join(tmpdir(), "k7-served-render-"));
const work = join(scratch, "work");
const home = join(scratch, "home");
for (const d of [work, home, join(scratch, "settings")]) mkdirSync(d, { recursive: true });
const env = {
  ...process.env,
  STUDIO_HOME: home,
  STUDIO_ENGINE_ROOTS: work,
  STUDIO_ENGINE_SILENT: "1",
  K7_SETTINGS_DIR: join(scratch, "settings"),
};

let failures = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"}  ${what}`);
  if (!ok) failures++;
};

/** A sine of `hz` for `seconds`, stereo 16-bit at 48 kHz, written under the engine's roots. */
function tone(name: string, hz: number, seconds: number): string {
  const rate = 48000;
  const frames = rate * seconds;
  const data = Buffer.alloc(frames * 4);
  for (let f = 0; f < frames; f++) {
    const v = Math.round(Math.sin((2 * Math.PI * hz * f) / rate) * 0.5 * 32767);
    data.writeInt16LE(v, f * 4);
    data.writeInt16LE(v, f * 4 + 2);
  }
  const file = join(work, name);
  writeFileSync(file, wavFile(data, rate, 2, 16));
  return file;
}

const audioTrack = (id: string, source: string) =>
  trackDoc(id, { clips: [clipDoc({ id: `${id}c`, type: "audio", name: id, source })] });

const synthTrack = (id: string, key: number, beats: number) =>
  trackDoc(id, {
    contentType: "midi",
    clips: [
      clipDoc({
        id: `${id}c`,
        type: "midi",
        name: id,
        length: beats,
        notes: [{ time: 0, duration: beats, key, vel: 0.8, channel: 1 }],
      }),
    ],
    plugins: [pluginDoc({ id: `${id}:synth`, type: "4osc", name: "synth", role: "instrument" })],
  });

/** A free loopback port: never one of the user's. */
async function freePort(): Promise<number> {
  return await new Promise((ok) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const port = (s.address() as { port: number }).port;
      s.close(() => ok(port));
    });
  });
}

/** `--render` on its own process, as the supervisor's job runs it: the reference. */
function cliRender(doc: ProjectDoc, out: string, stems: boolean) {
  const file = join(scratch, `${Math.random().toString(36).slice(2)}.json`);
  writeFileSync(file, JSON.stringify(doc));
  const r = spawnSync(bin as string, ["--render", file, "--out", out, ...(stems ? ["--stems"] : [])], {
    env,
    encoding: "utf8",
  });
  if (r.status !== 0) throw new Error(`--render failed (${r.status}): ${r.stderr}`);
  return JSON.parse(r.stdout.trim().split("\n").pop() as string);
}

/** The largest difference between two wavs' samples, in dBFS; -Infinity when they are equal. */
function diffDb(a: string, b: string): number {
  const x = readWav(a);
  const y = readWav(b);
  if (x.frames !== y.frames || x.channels !== y.channels) return Number.POSITIVE_INFINITY;
  let max = 0;
  for (let f = 0; f < x.frames; f++)
    for (let c = 0; c < x.channels; c++) max = Math.max(max, Math.abs(x.at(f, c) - y.at(f, c)));
  return max === 0 ? Number.NEGATIVE_INFINITY : 20 * Math.log10(max);
}

function peakDb(file: string): number {
  const w = readWav(file);
  let max = 0;
  for (let f = 0; f < w.frames; f++) for (let c = 0; c < w.channels; c++) max = Math.max(max, Math.abs(w.at(f, c)));
  return max === 0 ? Number.NEGATIVE_INFINITY : 20 * Math.log10(max);
}

type Rendered = { ok: boolean; busy?: boolean; error?: string; manifest?: { mix: string; stems: { name: string; file: string }[] } };

async function main() {
  const port = await freePort();
  const engine = spawn(bin as string, [String(port)], { env, stdio: ["ignore", "ignore", "pipe"] });
  const gone = new Promise<void>((ok) => engine.on("exit", () => ok()));
  let log = "";
  engine.stderr.on("data", (d: Buffer) => {
    log += d.toString();
  });
  const url = `ws://127.0.0.1:${port}`;
  let ws: WebSocket | null = null;
  for (let i = 0; i < 100 && !ws; i++) {
    ws = await new Promise<WebSocket | null>((ok) => {
      const s = new WebSocket(url);
      s.on("open", () => ok(s));
      s.on("error", () => setTimeout(() => ok(null), 100));
    });
  }
  if (!ws) throw new Error(`the engine on ${port} never answered:\n${log}`);
  const sock = ws;
  // The longest the message thread went unheard while asked, from `whoami`: a render must never hold it (docs/hangWatch.md).
  let silentMs = 0;
  const waiting = new Map<number, (r: Rendered) => void>();
  sock.on("message", (data: Buffer, isBinary: boolean) => {
    if (isBinary) return;
    const m = JSON.parse(data.toString());
    if (m.type === "rendered") waiting.get(m.id)?.(m);
    if (m.type === "whoami" && typeof m.messageThreadMs === "number") silentMs = Math.max(silentMs, m.messageThreadMs);
  });
  let next = 1;
  const render = (doc: unknown, outDir: string, stems = false, extra = {}): Promise<Rendered> => {
    const id = next++;
    const reply = new Promise<Rendered>((ok, fail) => {
      const timer = setTimeout(() => fail(new Error(`render ${id} got no answer`)), 150_000);
      waiting.set(id, (r) => {
        clearTimeout(timer);
        ok(r);
      });
    });
    sock.send(JSON.stringify({ type: "render", id, doc, outDir, stems, ...extra }));
    const died = gone.then((): Rendered => {
      throw new Error(`the engine exited (${engine.exitCode ?? engine.signalCode}) during render ${id}`);
    });
    return Promise.race([reply, died]);
  };

  try {
    // --- the same doc through both paths, stems included; sources only, since 4OSC starts at a random phase on every run ---
    const a = tone("a.wav", 220, 4);
    const b = tone("b.wav", 330, 3);
    const two = emptyDoc({ name: "two", tracks: [audioTrack("low", a), audioTrack("high", b)] });
    const cliOut = join(work, "cli");
    cliRender(two, cliOut, true);
    const servedOut = join(work, "served");
    const r = await render(two, servedOut, true);
    check(r.ok === true, `served render answered ok${r.error ? ` (${r.error})` : ""}`);
    const mixDiff = diffDb(join(cliOut, "mix.wav"), join(servedOut, "mix.wav"));
    console.log(`      mix --render vs served: ${mixDiff === Number.NEGATIVE_INFINITY ? "identical" : `${mixDiff.toFixed(1)} dB`}`);
    check(mixDiff <= -120, "the two mixes agree within -120 dB");
    const stems = r.manifest?.stems ?? [];
    check(stems.length === 2, `two stems in the manifest (${stems.length})`);
    if (stems.length === 2) {
      const between = diffDb(join(servedOut, stems[0].file), join(servedOut, stems[1].file));
      check(between > -60, `the two stems differ (${between.toFixed(1)} dB apart)`);
      for (const s of stems)
        check(diffDb(join(cliOut, s.file), join(servedOut, s.file)) <= -120, `stem ${s.name} agrees with --render's`);
    }

    // --- an mp3 source: its proxy is started by a timer only a running message loop fires ---
    const mp3 = join(work, "a.mp3");
    const enc = spawnSync("ffmpeg", ["-loglevel", "error", "-y", "-i", a, "-codec:a", "libmp3lame", mp3]);
    if (enc.status !== 0) throw new Error("ffmpeg could not make the mp3 source this test needs");
    const mp3Out = join(work, "mp3");
    const m = await render(emptyDoc({ name: "mp3", tracks: [audioTrack("mp3", mp3)] }), mp3Out);
    check(m.ok === true && peakDb(join(mp3Out, "mix.wav")) > -12, `an mp3-sourced doc renders${m.error ? ` (${m.error})` : ""}`);

    // --- one at a time ---
    const long = emptyDoc({ name: "long", tracks: [synthTrack("s1", 45, 120), synthTrack("s2", 52, 120)] });
    const first = render(long, join(work, "first"), true);
    const second = await render(long, join(work, "second"));
    check(second.ok === false && second.busy === true, `a second render is answered busy${second.error ? ` (${second.error})` : ""}`);
    check(!existsSync(join(work, "second")), "the busy one wrote nothing");
    check((await first).ok === true, "the first one still finishes");

    // --- hostile documents ---
    const outsideOut = join(work, "outside");
    const outside = await render(emptyDoc({ tracks: [audioTrack("x", fileURLToPath(import.meta.url))] }), outsideOut);
    check(outside.ok === false && /outside/.test(outside.error ?? ""), `a source outside the roots is refused (${outside.error})`);
    check(!existsSync(outsideOut), "…and nothing was written");
    const quietOut = join(work, "quiet");
    const quiet = await render(emptyDoc({ name: "quiet" }), quietOut);
    check(quiet.ok === false && /no audio/.test(quiet.error ?? ""), `a doc with nothing audible passes the engine's error (${quiet.error})`);
    check(!existsSync(join(quietOut, "mix.wav")), "…and leaves no mix");
    const notMine = await render(two, join(scratch, "elsewhere"));
    check(notMine.ok === false && /outside/.test(notMine.error ?? ""), `an outDir outside the write roots is refused (${notMine.error})`);
    const based = await render(emptyDoc({ tracks: [audioTrack("tone", "a.wav")] }), join(work, "based"), false, { base: work });
    check(based.ok === true, `a relative source under \`base\` renders${based.error ? ` (${based.error})` : ""}`);
    const climb = await render(emptyDoc({ tracks: [audioTrack("tone", `${"../".repeat(30)}etc/hosts`)] }), join(work, "climb"), false, { base: work });
    check(climb.ok === false && /outside/.test(climb.error ?? ""), `a relative source climbing out of \`base\` is refused (${climb.error})`);

    // --- the playing stream while renders run ---
    const seconds = 30;
    const meter = spawn(process.execPath, [join(here, "measure-broadcast.mjs"), url, String(seconds)], {
      stdio: ["ignore", "pipe", "inherit"],
    });
    let measured = "";
    meter.stdout.on("data", (d: Buffer) => {
      measured += d.toString();
    });
    const done = new Promise<void>((ok) => meter.on("exit", () => ok()));
    await new Promise((ok) => setTimeout(ok, 3000));
    const until = Date.now() + (seconds - 8) * 1000;
    let renders = 0;
    let rendering = 0;
    silentMs = 0;
    const probe = setInterval(() => sock.send(JSON.stringify({ type: "whoami" })), 100);
    while (Date.now() < until) {
      const t0 = Date.now();
      const x = await render(long, join(work, `load${renders}`), true);
      if (!x.ok) throw new Error(`a render under load failed: ${x.error}`);
      rendering += Date.now() - t0;
      renders++;
    }
    clearInterval(probe);
    await done;
    const ratio = Number(/realtime ratio\s+([\d.]+)/.exec(measured)?.[1] ?? Number.NaN);
    const dropped = Number(/dropped by sender\s+(\d+)/.exec(measured)?.[1] ?? Number.NaN);
    console.log(`      ${renders} renders, ${(rendering / 1000).toFixed(1)}s of rendering in a ${seconds}s stream`);
    check(Math.abs(ratio - 1) <= 0.001, `the stream's realtime ratio stays 1 ± 0.001 (${ratio})`);
    check(dropped === 0, `no frame dropped by the sender (${dropped})`);
    check(silentMs < 1000, `the message thread kept answering during the renders (silent at most ${silentMs.toFixed(0)} ms)`);
  } catch (e) {
    failures++;
    throw e;
  } finally {
    sock.close();
    // A wedged engine ignores SIGTERM, and a promise nothing else keeps alive would end this script silently.
    engine.kill("SIGTERM");
    const kill = setTimeout(() => engine.kill("SIGKILL"), 5000);
    await gone;
    clearTimeout(kill);
    if (failures === 0) rmSync(scratch, { recursive: true, force: true });
    else console.log(`scratch kept: ${scratch} (${readdirSync(work).join(", ")})\n--- engine log ---\n${log}`);
  }
  console.log(failures === 0 ? "\nall served-render checks pass" : `\n${failures} check(s) failed`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e: Error) => {
  console.log(`FAIL  ${e.message}`);
  process.exit(1);
});
