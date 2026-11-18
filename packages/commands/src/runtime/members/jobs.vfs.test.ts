// jobs.vfs.test.ts — a host member's calls: the flag set before the request, the codes mapped to the errors a verb catches, the poll.

import { describe, expect, it } from "vitest";
import { isNoFfmpeg } from "../../hostTypes.ts";
import type { RunFlags } from "../host.ts";
import { fakeServer } from "../testing/fakeServer.ts";
import { Vfs } from "../vfs.ts";
import {
  adoptFiles,
  HostError,
  type Hosting,
  hostCall,
  hostJob,
} from "./jobs.ts";

function bed() {
  const server = fakeServer(new Vfs(true), "/ws", "song.k7");
  const flags: RunFlags = { reload: false, hostCalled: false };
  const h: Hosting = { remote: server, flags, pollMs: 1 };
  return { server, flags, h };
}

describe("hostCall", () => {
  it("marks the run before the request, so a refusal still counts as a call", async () => {
    const { server, flags, h } = bed();
    let seen = false;
    server.serve("peak", () => {
      seen = flags.hostCalled;
      return { status: 200, body: { db: -3 } };
    });
    expect(await hostCall(h, "peak", { file: "/w/a.wav" })).toEqual({ db: -3 });
    expect(seen).toBe(true);
    flags.hostCalled = false;
    await expect(hostCall(h, "nothing", {})).rejects.toBeInstanceOf(HostError);
    expect(flags.hostCalled).toBe(true);
  });

  it("sends the runtime's own project as `at`, whatever the verb put there", async () => {
    const { server, h } = bed();
    let seen: unknown;
    server.serve("render", (body) => {
      seen = body;
      return { status: 200, body: {} };
    });
    await hostCall({ ...h, at: "song.k7" }, "render", {
      at: "/w/song.k7",
      doc: 1,
    });
    expect(seen).toEqual({ at: "song.k7", doc: 1 });
  });

  it("throws the server's sentence with its code and status", async () => {
    const { server, h } = bed();
    server.serve("devices", () => ({
      status: 503,
      body: { error: "no engine", code: "no-engine" },
    }));
    const e = await hostCall(h, "devices", null).catch((x) => x);
    expect(e).toBeInstanceOf(HostError);
    expect(e).toMatchObject({
      message: "no engine",
      code: "no-engine",
      status: 503,
    });
  });

  it("throws the class a verb catches for `no-ffmpeg`", async () => {
    const { server, h } = bed();
    server.serve("probe", () => ({
      status: 503,
      body: { error: "nothing reads it", code: "no-ffmpeg" },
    }));
    const e = await hostCall(h, "probe", { file: "/w/a.mp4" }).catch((x) => x);
    expect(isNoFfmpeg(e)).toBe(true);
  });

  it("words a refusal with no body by its status", async () => {
    const { server, h } = bed();
    server.serve("peak", () => ({ status: 500, body: {} }));
    await expect(hostCall(h, "peak", {})).rejects.toThrow(
      "the server answered 500",
    );
  });
});

describe("hostJob", () => {
  it("polls until the job is done and answers its value and id", async () => {
    const { server, flags, h } = bed();
    const job = server.startJob([
      { state: "running", fraction: 0.2 },
      { state: "running", fraction: 0.8 },
      { state: "done", value: { files: [] } },
    ]);
    server.serve("render", () => ({ status: 202, body: { job } }));
    const done = await hostJob(h, "render", { at: "song.k7" });
    expect(done).toEqual({ job, value: { files: [] } });
    expect(flags.hostCalled).toBe(true);
  });

  it("throws a failed job as a refused request, with its code", async () => {
    const { server, h } = bed();
    const job = server.startJob([
      { state: "failed", error: "no machine", code: "no-machine" },
    ]);
    server.serve("separate", () => ({ status: 202, body: { job } }));
    await expect(hostJob(h, "separate", {})).rejects.toMatchObject({
      message: "no machine",
      code: "no-machine",
    });
  });

  it("says a forgotten job is gone", async () => {
    const { server, h } = bed();
    server.serve("render", () => ({ status: 202, body: { job: "nope" } }));
    await expect(hostJob(h, "render", {})).rejects.toMatchObject({
      code: "gone",
    });
  });

  it("stops polling when the runtime closes, leaving the job to the heartbeat", async () => {
    const { server, h } = bed();
    const job = server.startJob([{ state: "running" }]);
    let polls = 0;
    const remote = {
      ...server,
      job: async (id: string) => {
        polls++;
        return server.job(id);
      },
    };
    server.serve("render", () => ({ status: 202, body: { job } }));
    const stop = new AbortController();
    const run = hostJob(
      { ...h, remote, signal: stop.signal, pollMs: 5 },
      "render",
      {},
    );
    await new Promise((ok) => setTimeout(ok, 30));
    stop.abort();
    await expect(run).rejects.toMatchObject({ code: "closed" });
    const seen = polls;
    await new Promise((ok) => setTimeout(ok, 30));
    expect(polls).toBe(seen);
  });

  it("does not start polling for a run already closed", async () => {
    const { server, h } = bed();
    const job = server.startJob([{ state: "running" }]);
    let asked = 0;
    server.serve("render", () => {
      asked++;
      return { status: 202, body: { job } };
    });
    const stop = new AbortController();
    stop.abort();
    await expect(
      hostJob({ ...h, signal: stop.signal }, "render", {}),
    ).rejects.toMatchObject({ code: "closed" });
    expect(asked).toBe(0);
    expect(h.flags.hostCalled).toBe(false);
  });
});

describe("adoptFiles", () => {
  it("lands each file where the member places it, with its seconds", () => {
    const vfs = new Vfs(true);
    vfs.mkdir("/w/p.k7", true);
    adoptFiles(
      vfs,
      "j1",
      [
        { out: "a.wav", len: 8, seconds: 2 },
        { out: "b.wav", len: 4, seconds: null },
      ],
      (out) => `/w/p.k7/${out}`,
    );
    expect(vfs.secondsOf("/w/p.k7/a.wav")).toBe(2);
    expect(vfs.secondsOf("/w/p.k7/b.wav")).toBeNull();
    expect(vfs.staged().map((o) => o.kind)).toEqual(["adopt", "adopt"]);
  });
});
