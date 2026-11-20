// jobs.test.ts — a jobs channel end to end: the agent answers the request ITSELF, the file it
// names is on the machine before the handler runs, and a file the handler writes is on the
// server before the reply naming it arrives. The handler here stands in for a separation.

import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import { JobError, requestJob } from "./jobsClient.ts";
import { type AgentHandle, agentKinds } from "./testAgents.ts";
import {
  describeCore,
  type HubTarget,
  startHub,
  until as untilMs,
} from "./testCore.ts";

const until = (ok: () => boolean | Promise<boolean>) => untilMs(ok, 5000);

describeCore.each(agentKinds)("a jobs channel — $name agent", ({ make }) => {
  let hub: HubTarget;
  let agent: AgentHandle;
  let remote: string;
  let port: number;
  let cache: string;

  beforeEach(async () => {
    remote = mkdtempSync(join(tmpdir(), "jobs-remote-"));
    cache = mkdtempSync(join(tmpdir(), "jobs-cache-"));
    hub = await startHub();
    port = hub.port;
    agent = make({
      registryUrl: hub.agentUrl,
      id: "e1",
      token: "t",
      enginePort: 1, // no engine at all: a jobs channel never dials one
      cacheDir: cache,
      log: () => {},
      jobs: {
        // `where` answers one path twice (the server's, and the machine's under `raw`); `upper` writes a "stem" beside its source.
        where(req, emit) {
          emit({ type: "where", path: req.source, raw: { path: req.source } });
        },
        async upper(req, emit) {
          const text = readFileSync(String(req.source), "utf8");
          emit({ type: "progress", percent: 50 });
          mkdirSync(String(req.outDir), { recursive: true });
          const out = join(
            String(req.outDir),
            `${basename(String(req.source))}.up`,
          );
          writeFileSync(out, text.toUpperCase());
          emit({ type: "done", files: [out] });
        },
      },
    });
    agent.start();
    await until(() => hub.has("e1"));
  });

  afterEach(async () => {
    agent.stop();
    await hub.stop();
  });

  const roots = () => [{ remote, name: "lib", writable: true }];

  it("runs the request on the machine and brings its output back", async () => {
    writeFileSync(join(remote, "song.txt"), "la la");
    const ch = await hub.openReady("e1", roots(), 5000, "jobs");
    if (!ch) throw new Error("no channel");
    const seen: string[] = [];
    const reply = await requestJob(
      ch,
      {
        type: "upper",
        source: `${remote}/song.txt`,
        outDir: `${remote}/stems`,
      },
      (r) => r.type === "done",
      (r) => seen.push(r.type),
    );
    expect(seen).toEqual(["progress", "done"]);
    expect(reply.files).toEqual([`${remote}/stems/song.txt.up`]);
    expect(readFileSync(join(remote, "stems", "song.txt.up"), "utf8")).toBe(
      "LA LA",
    );
  });

  it("delivers a reply's `raw` as the machine wrote it, and maps the rest", async () => {
    writeFileSync(join(remote, "w.txt"), "w");
    const ch = await hub.openReady("e1", roots(), 5000, "jobs");
    if (!ch) throw new Error("no channel");
    const reply = await requestJob(
      ch,
      { type: "where", source: `${remote}/w.txt` },
      (r) => r.type === "where",
    );
    expect(reply.path).toBe(`${remote}/w.txt`);
    const local = (reply.raw as { path: string }).path;
    expect(local).toBe(join(cache, "lib", "w.txt"));
    expect(readFileSync(local, "utf8")).toBe("w");
  });

  it("says so when the machine does not do what was asked", async () => {
    const ch = await hub.openReady("e1", roots(), 5000, "jobs");
    if (!ch) throw new Error("no channel");
    await expect(requestJob(ch, { type: "nope" }, () => true)).rejects.toThrow(
      JobError,
    );
  });

  it("is reachable from a bound loopback path, as a hosted file service reaches it", async () => {
    writeFileSync(join(remote, "b.txt"), "b");
    const path = await hub.bindLocal("e1", roots(), "jobs");
    const ws = new WebSocket(`ws://127.0.0.1:${port}${path}`);
    const reply = await requestJob(
      ws,
      { type: "upper", source: `${remote}/b.txt`, outDir: `${remote}/out` },
      (r) => r.type === "done",
    );
    expect(reply.files).toEqual([`${remote}/out/b.txt.up`]);
    ws.close();
  });
});
