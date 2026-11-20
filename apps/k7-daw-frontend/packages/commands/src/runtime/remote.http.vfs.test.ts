// remote.http.vfs.test.ts — `httpRemote`'s host routes over a real socket: the method, the body, the 404 as `gone`, and a poll that is asked again.

import { afterEach, describe, expect, it } from "vitest";
import { httpRemote } from "./remote.ts";
import { fakeServer } from "./testing/fakeServer.ts";
import { serveHttp } from "./testing/httpServer.ts";
import { Vfs } from "./vfs.ts";

let stop: (() => Promise<void>) | undefined;
afterEach(async () => {
  await stop?.();
  stop = undefined;
});

async function over() {
  const server = fakeServer(new Vfs(true), "/ws", "song.k7");
  const http = await serveHttp(server);
  stop = http.close;
  return { server, base: http.base, remote: httpRemote({ base: http.base }) };
}

describe("httpRemote's host routes", () => {
  it("posts a body, gets with a query, and answers a status with its body", async () => {
    const { server, remote } = await over();
    const seen: unknown[] = [];
    server.serve("peak", (body, query) => {
      seen.push([body, query]);
      return { status: 202, body: { job: "j" } };
    });
    server.serve("plugins", (body, query) => {
      seen.push([body, query]);
      return { status: 200, body: { plugins: [] } };
    });
    expect(await remote.host("peak", { file: "/w/a.wav" })).toEqual({
      status: 202,
      body: { job: "j" },
    });
    expect(await remote.host("plugins", null, "filter=a")).toEqual({
      status: 200,
      body: { plugins: [] },
    });
    expect(seen).toEqual([
      [{ file: "/w/a.wav" }, ""],
      [null, "filter=a"],
    ]);
  });

  it("posts a path with a leading slash to that route, outside /verb-host", async () => {
    const { server, remote } = await over();
    server.serve("/bench/save", (body) => ({
      status: 200,
      body: { got: body },
    }));
    expect(await remote.host("/bench/save", { dir: "/b" })).toEqual({
      status: 200,
      body: { got: { dir: "/b" } },
    });
  });

  it("allows a leading slash for the bench's save alone, and a double slash under /verb-host reaches nothing", async () => {
    const { server, base, remote } = await over();
    server.serve("/anything", () => ({ status: 200, body: {} }));
    await expect(remote.host("/anything", {})).rejects.toThrow(
      "not a host route",
    );
    const wrong = await fetch(`${base}/verb-host//bench/save`, {
      method: "POST",
      body: "{}",
    });
    expect(wrong.status).toBe(404);
  });

  it("reads the 404 of a forgotten or foreign id as `gone`, and a running job as it is", async () => {
    const { server, remote } = await over();
    const job = server.startJob([{ state: "running", fraction: 0.5 }]);
    expect(await remote.job(job)).toEqual({ state: "running", fraction: 0.5 });
    expect(await remote.job("0000")).toMatchObject({
      state: "failed",
      code: "gone",
    });
  });
});

describe("httpRemote's poll", () => {
  const retry = { forMs: 200, everyMs: 1 };
  const answer = (status: number, body: unknown) =>
    new Response(JSON.stringify(body), { status });

  it("asks again after a 502, a 503 or a lost connection, and answers what comes", async () => {
    const replies: (Response | Error)[] = [
      answer(502, {}),
      new TypeError("fetch failed"),
      answer(503, {}),
      answer(200, { state: "done", value: { files: [] } }),
    ];
    let asked = 0;
    const fetchStub = (async () => {
      const r = replies[asked++];
      if (r instanceof Error) throw r;
      return r;
    }) as unknown as typeof fetch;
    const remote = httpRemote({ base: "http://x", fetch: fetchStub, retry });
    expect(await remote.job("j")).toEqual({
      state: "done",
      value: { files: [] },
    });
    expect(asked).toBe(4);
  });

  it("gives up on a server that stays away, and does not retry a refusal", async () => {
    const gone = (async () => {
      throw new TypeError("fetch failed");
    }) as unknown as typeof fetch;
    await expect(
      httpRemote({ base: "http://x", fetch: gone, retry }).job("j"),
    ).rejects.toThrow("/verb-host/job");
    let asked = 0;
    const refusing = (async () => {
      asked++;
      return answer(500, { error: "broken" });
    }) as unknown as typeof fetch;
    expect(
      await httpRemote({ base: "http://x", fetch: refusing, retry }).job("j"),
    ).toMatchObject({ state: "failed", error: "broken" });
    expect(asked).toBe(1);
  });
});
