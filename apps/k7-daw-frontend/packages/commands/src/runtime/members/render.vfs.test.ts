// render.vfs.test.ts — the render and media members against the fake server: the bodies sent, the files adopted, the codes mapped.

import { describe, expect, it } from "vitest";
import { isNoFfmpeg } from "../../hostTypes.ts";
import type { RunFlags } from "../host.ts";
import { fakeServer } from "../testing/fakeServer.ts";
import { Vfs } from "../vfs.ts";
import type { Hosting } from "./jobs.ts";
import { mediaMembers, takeMember } from "./media.ts";
import { renderMembers } from "./render.ts";

function bed() {
  const vfs = new Vfs(true);
  const server = fakeServer(vfs, "/ws", "song.k7");
  const flags: RunFlags = { reload: false, hostCalled: false };
  const h: Hosting = { remote: server, flags, pollMs: 1, at: "song.k7" };
  return {
    vfs,
    server,
    flags,
    render: renderMembers(h, vfs),
    media: mediaMembers(h, vfs),
    take: takeMember(h, () => "/w/song.k7/clips", "/w/song.k7"),
  };
}

describe("the render members", () => {
  it("a render sends the doc and its options, and adopts the job's files beside the manifest", async () => {
    const { vfs, server, render } = bed();
    let sent: unknown;
    const job = server.startJob([
      {
        state: "done",
        value: {
          manifest: { mix: "mix.wav", stems: [{ name: "a", file: "a.wav" }] },
          files: [
            { out: "mix.wav", len: 8, seconds: 1 },
            { out: "a.wav", len: 4, seconds: 0.5 },
          ],
        },
      },
    ]);
    server.serve("render", (body) => {
      sent = body;
      return { status: 202, body: { job } };
    });
    const doc = { name: "t" } as never;
    const got = await render.render?.(doc, "/tmp/r", {
      stems: true,
      base: "/w/song.k7",
    });
    expect(got).toEqual({
      mix: "mix.wav",
      stems: [{ name: "a", file: "a.wav" }],
    });
    expect(sent).toEqual({
      at: "song.k7",
      doc,
      stems: true,
      base: "/w/song.k7",
    });
    expect(vfs.secondsOf("/tmp/r/mix.wav")).toBe(1);
    expect(vfs.secondsOf("/tmp/r/a.wav")).toBe(0.5);
  });

  it("a render without stems asks for none", async () => {
    const { server, render } = bed();
    let sent: Record<string, unknown> | null = null;
    const job = server.startJob([
      {
        state: "done",
        value: { manifest: { mix: "m.wav", stems: [] }, files: [] },
      },
    ]);
    server.serve("render", (body) => {
      sent = body;
      return { status: 202, body: { job } };
    });
    await render.render?.({} as never, "/tmp/r");
    expect(sent).not.toHaveProperty("stems");
    expect(sent).not.toHaveProperty("base");
  });

  it("a peak is the server's db, a cut adopts its files in the folder named, and a failure is the server's sentence", async () => {
    const { vfs, server, render } = bed();
    server.serve("peak", () => ({ status: 200, body: { db: -6 } }));
    expect(await render.peak?.("/w/a.wav")).toBe(-6);
    const job = server.startJob([
      {
        state: "done",
        value: { files: [{ out: "drums.wav", len: 4, seconds: 1 }] },
      },
    ]);
    server.serve("separate", () => ({ status: 202, body: { job } }));
    const files = await render.runColdStep?.({
      step: { step: "split", model: "/s/models/m.onnx", outputs: ["drums"] },
      source: "/w/song.k7/a.wav",
      outDir: "/w/song.k7/stems",
    });
    expect(files).toEqual(["/w/song.k7/stems/drums.wav"]);
    expect(vfs.secondsOf("/w/song.k7/stems/drums.wav")).toBe(1);
    server.serve("separate", () => ({
      status: 503,
      body: { error: "no engine", code: "no-machine" },
    }));
    await expect(
      render.runColdStep?.({
        step: { step: "s", model: "/m", outputs: ["drums"] },
        source: "/a",
        outDir: "/o",
      }),
    ).rejects.toMatchObject({ code: "no-machine" });
  });

  it("a playlist's stems come back as the server states them", async () => {
    const { server, render } = bed();
    const songs = [{ song: "a", state: "done", files: ["drums.wav"] }];
    const job = server.startJob([{ state: "done", value: { songs } }]);
    server.serve("playlist-stems", () => ({ status: 202, body: { job } }));
    const got = await render.playlistStems?.({
      root: "/w",
      songs: ["a"],
      steps: [{ step: "s", model: "/m", outputs: ["drums"] }],
    });
    expect(got).toEqual({ songs });
  });
});

describe("the media members", () => {
  it("a probe answers the facts, null for no video, and the class a verb catches for no ffmpeg", async () => {
    const { server, media } = bed();
    server.serve("probe", () => ({
      status: 200,
      body: { facts: { codec: "h264" } },
    }));
    expect(await media.probe?.("/w/a.mp4")).toEqual({ codec: "h264" });
    server.serve("probe", () => ({ status: 200, body: { facts: null } }));
    expect(await media.probe?.("/w/a.mp3")).toBeNull();
    server.serve("probe", () => ({
      status: 503,
      body: { error: "needs ffmpeg", code: "no-ffmpeg" },
    }));
    const e = await media.probe?.("/w/a.mp4").catch((x) => x);
    expect(isNoFfmpeg(e)).toBe(true);
  });

  it("an extract adopts the sound at the path asked, and a silent film leaves nothing", async () => {
    const { vfs, server, media } = bed();
    const job = server.startJob([
      {
        state: "done",
        value: {
          facts: { frames: 8, sampleRate: 8000, channels: 1 },
          files: [{ out: "sound.wav", len: 40, seconds: 0.001 }],
        },
      },
    ]);
    server.serve("extract", () => ({ status: 202, body: { job } }));
    const facts = await media.extractSound?.(
      "/w/song.k7/a.mp4",
      "/w/song.k7/a.wav",
    );
    expect(facts).toEqual({ frames: 8, sampleRate: 8000, channels: 1 });
    expect(vfs.secondsOf("/w/song.k7/a.wav")).toBe(0.001);
    const quiet = server.startJob([
      { state: "done", value: { facts: null, files: [] } },
    ]);
    server.serve("extract", () => ({ status: 202, body: { job: quiet } }));
    expect(
      await media.extractSound?.("/w/song.k7/b.mp4", "/w/song.k7/b.wav"),
    ).toBeNull();
    expect(vfs.stat("/w/song.k7/b.wav")).toBeUndefined();
  });

  it("a prepare sends the clip, its source and the copies asked, and answers whether it started", async () => {
    const { server, media, flags } = bed();
    const bodies: unknown[] = [];
    server.serve("prepare", (body) => {
      bodies.push(body);
      return { status: 200, body: { started: bodies.length === 1 } };
    });
    const ask = {
      at: "/w/song.k7",
      clip: "main",
      source: "media/a.mov",
      optimized: "transcode" as const,
      proxy: true,
    };
    expect(await media.prepare?.(ask)).toEqual({ started: true });
    expect(await media.prepare?.(ask)).toEqual({ started: false });
    // The server's own name for the project is sent, never the verb's virtual song path.
    expect(bodies[0]).toEqual({
      at: "song.k7",
      clip: "main",
      source: "media/a.mov",
      optimized: "transcode",
      proxy: true,
    });
    expect(flags.hostCalled).toBe(true);
    server.serve("prepare", () => ({
      status: 403,
      body: { error: "'../x' is outside the folders you read" },
    }));
    await expect(media.prepare?.(ask)).rejects.toThrow("outside the folders");
  });

  it("a transcription is the job's srt, null for no speech, and a failure is whisper's sentence", async () => {
    const { server, media } = bed();
    let sent: unknown;
    const srt = "1\n00:00:01,000 --> 00:00:02,000\nhello\n";
    const job = server.startJob([
      { state: "running" },
      { state: "done", value: { srt } },
    ]);
    server.serve("transcribe", (body) => {
      sent = body;
      return { status: 202, body: { job } };
    });
    expect(await media.transcribe?.("/w/song.k7/a.mp4")).toBe(srt);
    expect(sent).toEqual({ at: "song.k7", video: "/w/song.k7/a.mp4" });
    const quiet = server.startJob([{ state: "done", value: { srt: null } }]);
    server.serve("transcribe", () => ({ status: 202, body: { job: quiet } }));
    expect(await media.transcribe?.("/w/song.k7/b.mp4")).toBeNull();
    const broken = server.startJob([
      { state: "failed", error: "no whisper on this machine" },
    ]);
    server.serve("transcribe", () => ({ status: 202, body: { job: broken } }));
    await expect(media.transcribe?.("/w/song.k7/c.mp4")).rejects.toThrow(
      "no whisper on this machine",
    );
  });

  it("a picture sends the scene, the format, the settings and the sound, and answers the film's path once the job ends", async () => {
    const { server, media, flags } = bed();
    let sent: unknown;
    const job = server.startJob([
      { state: "running" },
      { state: "done", value: { path: "/films/song.mp4" } },
    ]);
    server.serve("picture", (body) => {
      sent = body;
      return { status: 202, body: { job } };
    });
    const scene = { viewport: [1280, 720], fps: 30, durationSec: 2 } as never;
    const settings = { size: { height: 360 }, quality: "max" as const };
    expect(
      await media.renderPicture?.({
        into: "/films",
        scene,
        format: "mp4",
        settings,
        audio: true,
      }),
    ).toBe("/films/song.mp4");
    expect(sent).toEqual({
      at: "song.k7",
      into: "/films",
      scene,
      format: "mp4",
      settings,
      audio: true,
    });
    expect(flags.hostCalled).toBe(true);
    const busy = server.startJob([
      { state: "failed", error: "no Chromium to render in" },
    ]);
    server.serve("picture", () => ({ status: 202, body: { job: busy } }));
    await expect(
      media.renderPicture?.({
        into: "/films",
        scene,
        format: "webm",
        settings: {},
        audio: false,
      }),
    ).rejects.toThrow("no Chromium");
    server.serve("picture", () => ({
      status: 409,
      body: { error: "a picture render is already running" },
    }));
    await expect(
      media.renderPicture?.({
        into: "/films",
        scene,
        format: "webm",
        settings: {},
        audio: false,
      }),
    ).rejects.toThrow("already running");
  });

  it("a take is filed from the record directory, its picture named under the song, and a refusal is the server's sentence", async () => {
    const { server, take, flags } = bed();
    let sent: unknown;
    server.serve("take/save", (body) => {
      sent = body;
      return { status: 200, body: { path: "recordings/d/t1.wav" } };
    });
    expect(
      await take.saveTake?.({ file: "t1.wav", video: "clips/up.webm" }),
    ).toBe("recordings/d/t1.wav");
    expect(sent).toEqual({
      at: "song.k7",
      recordDir: "/w/song.k7/clips",
      file: "t1.wav",
      video: "/w/song.k7/clips/up.webm",
    });
    expect(flags.hostCalled).toBe(true);
    server.serve("take/save", () => ({
      status: 500,
      body: { error: "not one of this project's takes" },
    }));
    await expect(take.saveTake?.({ file: "x.wav" })).rejects.toThrow(
      "not one of this project's takes",
    );
  });

  it("the envelope is the whole source's rms as peaks, and null on any failure", async () => {
    const { server, media } = bed();
    server.serve("waveform", () => ({
      status: 200,
      body: { rms: [0.1, 0.5], seconds: 2 },
    }));
    expect(await media.clipEnvelope?.("c1")).toEqual({
      peak: [0.1, 0.5],
      seconds: 2,
    });
    server.serve("waveform", () => ({ status: 404, body: {} }));
    expect(await media.clipEnvelope?.("c1")).toBeNull();
    server.serve("waveform", () => ({
      status: 200,
      body: { rms: [], seconds: 2 },
    }));
    expect(await media.clipEnvelope?.("c1")).toBeNull();
  });
});
