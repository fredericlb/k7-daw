// renderPage.ts — the page a SERVER render runs in headless Chromium: the same `exportScene` a
// browser export calls, fed by the job server that started it and reporting back to it. It knows
// nothing of accounts or projects: the job server hands it a scene whose URLs are its own, and it
// takes the finished file. See docs/serverRender.md.

import { provideWasmBytes, wasmReady } from "@studio/k7-wasm/boot";
import wasmBytes from "@studio/k7-wasm/module.wasm";
import type {
  ExportFormat,
  ExportQuality,
} from "@studio/song-schema/renderSettings";
import { exportScene } from "./export";
import type { VideoScene } from "./scene";

/** What the job server hands the page: the scene, and the render's settings. */
export interface RenderJob {
  scene: VideoScene;
  format: ExportFormat;
  audio: boolean;
  from?: number;
  frameCount?: number;
  size?: [number, number];
  quality?: ExportQuality;
}

// Every route is relative to the page, which the job server serves under its own one-off path.
const post = (path: string, body: BodyInit, type: string) =>
  fetch(path, { method: "POST", body, headers: { "content-type": type } });

// The film goes back as base64 text in chunks: a binary body can break the DevTools event that carries it.
const CHUNK = 3 * 1024 * 1024;

function base64Of(bytes: Uint8Array): string {
  let text = "";
  for (let at = 0; at < bytes.length; at += 0x8000)
    text += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
  return btoa(text);
}

async function postFilm(buffer: ArrayBuffer, type: string): Promise<void> {
  const bytes = new Uint8Array(buffer);
  let at = 0;
  do {
    const end = Math.min(at + CHUNK, bytes.length);
    const last = end >= bytes.length ? 1 : 0;
    const url = `result?type=${encodeURIComponent(type)}&last=${last}`;
    const r = await post(url, base64Of(bytes.subarray(at, end)), "text/plain");
    if (!r.ok) throw new Error(`the server refused the film (${r.status})`);
    at = end;
  } while (at < bytes.length);
}

async function run(): Promise<void> {
  provideWasmBytes(wasmBytes);
  if (!(await wasmReady()))
    throw new Error("the render page's WebAssembly module did not load");
  const job = (await (await fetch("job.json")).json()) as RenderJob;
  let audio: AudioBuffer | undefined;
  if (job.audio) {
    const bytes = await (await fetch("mix.wav")).arrayBuffer();
    // An OFFLINE context decodes with no output device, which a server has none of.
    audio = await new OfflineAudioContext(2, 1, 48000).decodeAudioData(bytes);
  }
  const file = await exportScene(
    job.scene,
    {
      format: job.format,
      ...(audio ? { audio } : {}),
      ...(job.from !== undefined ? { from: job.from } : {}),
      ...(job.frameCount !== undefined ? { frameCount: job.frameCount } : {}),
      ...(job.size ? { size: job.size } : {}),
      ...(job.quality ? { quality: job.quality } : {}),
    },
    // Every 30th frame: enough to say how far a long render is, not a request per frame.
    (p) => {
      if (p.frame % 30 === 0 || p.frame === p.total)
        void post("progress", JSON.stringify(p), "application/json");
    },
  );
  await postFilm(file.buffer, file.mimeType);
}

run().catch((e: unknown) => {
  void post("error", e instanceof Error ? e.message : String(e), "text/plain");
});
