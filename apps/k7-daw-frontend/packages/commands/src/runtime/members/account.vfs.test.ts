// account.vfs.test.ts — the engine, plugin, device, library and bench members against a scripted server (3f, M9).

import { describe, expect, it } from "vitest";
import type { Output, RunFlags } from "../host.ts";
import { fakeServer } from "../testing/fakeServer.ts";
import { Vfs } from "../vfs.ts";
import { benchMembers } from "./bench.ts";
import { deviceMembers } from "./devices.ts";
import { engineMembers } from "./engine.ts";
import { HostError, type Hosting } from "./jobs.ts";
import { libraryMembers } from "./library.ts";
import { pluginMembers } from "./plugins.ts";

function bed() {
  const server = fakeServer(new Vfs(true), "/ws", "song.k7");
  const flags: RunFlags = { reload: false, hostCalled: false };
  const h: Hosting = { remote: server, flags, pollMs: 1 };
  const out: Output = [];
  return { server, flags, h, out };
}

describe("engine members", () => {
  it("send each op by its name and push the server's lines", async () => {
    const { server, h, out, flags } = bed();
    const seen: unknown[] = [];
    server.serve("engine", (body) => {
      seen.push(body);
      return body?.op === "arm"
        ? { status: 200, body: { armed: true, lines: ["slow"] } }
        : { status: 200, body: { lines: ["engine offline"] } };
    });
    const m = engineMembers(h, out);
    await m.transport("play" as never);
    expect(await m.arm("t1", true)).toBe(true);
    expect(
      await m.takeArm?.({ track: "t", eventBeats: 1, punch: { from: 0 } }),
    ).toBe(true);
    expect(await m.timelinePlay?.(null)).toBe(true);
    await m.showNativePluginUI?.("p1");
    expect(seen).toEqual([
      { op: "transport", action: "play" },
      { op: "arm", track: "t1", armed: true },
      { op: "takeArm", arm: { track: "t", eventBeats: 1, punch: { from: 0 } } },
      { op: "timelinePlay", id: null },
      { op: "showUi", id: "p1" },
    ]);
    expect(out).toEqual(["slow", "engine offline"]);
    expect(flags.hostCalled).toBe(true);
  });

  it("answers undefined for an arm the engine did not take", async () => {
    const { server, h, out } = bed();
    server.serve("engine", () => ({
      status: 200,
      body: { armed: null, lines: ["no answer"] },
    }));
    expect(await engineMembers(h, out).arm("t")).toBeUndefined();
    expect(out).toEqual(["no answer"]);
  });
});

describe("plugin, device and library members", () => {
  it("list plugins by filter and word the lines as the engine kit does", async () => {
    const { server, h, out } = bed();
    let query = "";
    server.serve("plugins", (_b, q) => {
      query = q;
      return {
        status: 200,
        body: {
          plugins: [
            {
              name: "Synth",
              format: "VST3",
              manufacturer: "Acme",
              role: "instrument",
            },
          ],
        },
      };
    });
    const m = pluginMembers(h, out);
    expect((await m.getPlugins("VST3")).length).toBe(1);
    expect(query).toBe("filter=VST3");
    await m.listPlugins("");
    expect(out).toEqual(["  [inst] Synth  [VST3]  Acme"]);
    server.serve("plugins/rescan", () => ({
      status: 200,
      body: { plugins: [] },
    }));
    await m.rescanPlugins(true);
    expect(out.at(-1)).toBe("scanned 0 plugin(s)");
  });

  it("says no plugins cached for an empty list", async () => {
    const { server, h, out } = bed();
    server.serve("plugins", () => ({ status: 200, body: { plugins: [] } }));
    await pluginMembers(h, out).listPlugins();
    expect(out).toEqual(["no plugins cached — run: k7 engine scan"]);
  });

  it("devices: the list, and a stereo pair heard only by an engine", async () => {
    const { server, h } = bed();
    server.serve("devices", () => ({
      status: 200,
      body: { devices: [{ name: "a" }] },
    }));
    server.serve("devices/stereo", () => ({
      status: 200,
      body: { heard: false },
    }));
    const m = deviceMembers(h);
    expect(await m.getAudioDevices?.()).toEqual([{ name: "a" }]);
    expect(await m.setInputStereo?.(1, true)).toBe(false);
  });

  it("library: onnx null without a machine", async () => {
    const { server, h } = bed();
    server.serve("onnx", () => ({ status: 200, body: { providers: null } }));
    const m = libraryMembers(h);
    expect(await m.onnxProviders?.()).toBeNull();
    for (const code of ["off", "no-machine"]) {
      server.serve("onnx", () => ({
        status: 503,
        body: { error: "nobody", code },
      }));
      expect(await m.onnxProviders?.(["cpu"])).toBeNull();
    }
    server.serve("onnx", () => ({
      status: 400,
      body: { error: "too many names" },
    }));
    await expect(m.onnxProviders?.(["cpu"])).rejects.toBeInstanceOf(HostError);
  });

  it("a refusal throws the server's sentence", async () => {
    const { h } = bed();
    await expect(deviceMembers(h).getAudioDevices?.()).rejects.toBeInstanceOf(
      HostError,
    );
  });
});

describe("bench member", () => {
  // The route resolves a project NAME to the bench; a folder, real or virtual, is what it refuses.
  const benchRoute = (body: Record<string, unknown> | null) =>
    body?.dir !== undefined || body?.at !== "live.k7"
      ? {
          status: 400,
          body: { error: "only the bench saves a chain" },
        }
      : { status: 200, body: { ref: "@acct/x", steps: 2, outside: [] } };

  it("posts the project's name, never the verb's virtual folder, and returns the saved chain", async () => {
    const { server, h } = bed();
    let seen: unknown;
    server.serve("/bench/save", (body) => {
      seen = body;
      return benchRoute(body);
    });
    const saved = await benchMembers({ ...h, at: "live.k7" }).saveBenchChain?.(
      "/w/live.k7",
      "x",
    );
    expect(saved).toEqual({ ref: "@acct/x", steps: 2, outside: [] });
    expect(seen).toEqual({ name: "x", at: "live.k7" });
  });

  it("carries the route's refusal, and a run with no project asks nothing", async () => {
    const { server, h } = bed();
    server.serve("/bench/save", () => ({
      status: 400,
      body: { error: "name the chain" },
    }));
    await expect(
      benchMembers({ ...h, at: "live.k7" }).saveBenchChain?.("/b"),
    ).rejects.toThrow("name the chain");
    expect(() => benchMembers(h).saveBenchChain?.("/b")).toThrow(
      "no project is open",
    );
  });
});
