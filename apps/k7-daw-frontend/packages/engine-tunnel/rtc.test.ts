// rtc.test.ts — the WebRTC path end to end, with node-datachannel's own RTCPeerConnection
// standing in for the browser: the offer crosses the tunnel, the supervisor answers, and frames
// larger than one fragment reach the engine and come back whole. Skipped where the optional
// native package is not installed.

import { mkdtempSync } from "node:fs";
import { createRequire } from "node:module";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { toFragments } from "@studio/web-protocol/rtc-frames";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { WebSocketServer } from "ws";
import { type AgentHandle, agentKinds } from "./testAgents.ts";
import { type HubTarget, RUST_CORE, startHub } from "./testCore.ts";

const available = (() => {
  try {
    createRequire(import.meta.url).resolve("node-datachannel");
    return true;
  } catch {
    return false;
  }
})();

/** The slice of a browser's peer connection this test drives; this package has no DOM types. */
interface BrowserPeer {
  createDataChannel(label: string): {
    binaryType: string;
    readyState: string;
    onmessage: ((ev: { data: unknown }) => void) | null;
    send(data: Uint8Array): void;
  };
  createOffer(): Promise<{ type: string; sdp?: string }>;
  setLocalDescription(d: { type: string; sdp?: string }): Promise<void>;
  setRemoteDescription(d: { type: string; sdp: string }): Promise<void>;
  localDescription: { sdp: string } | null;
  iceGatheringState: string;
  close(): void;
}

/** How long a data channel may take to open: the FIRST ICE of a run, on a loaded CI box, has taken over 8 s. */
const OPEN_MS = 20_000;

/** How long a peer waits for its ICE gathering before it offers what it has. */
const GATHER_MS = 3000;

/** Browsers that connect and hang up in turn, in the test below. */
const ROUNDS = 3;

const until = async (ok: () => boolean, ms = 8000): Promise<void> => {
  const end = Date.now() + ms;
  while (!ok()) {
    if (Date.now() > end) throw new Error("timed out");
    await new Promise((r) => setTimeout(r, 20));
  }
};

describe.skipIf(!available || !RUST_CORE).each(agentKinds.filter((k) => k.rtc))(
  "the WebRTC path — $name agent",
  ({ make }) => {
    let hub: HubTarget;
    let agent: AgentHandle;
    let engine: WebSocketServer;
    let engineGot: string[];

    // A fresh peer's offer answered by the hub; the polyfill sometimes adds the answer before its ICE transport exists.
    const offerPath = async (
      Peer: new (cfg: object) => BrowserPeer,
    ): Promise<{
      pc: BrowserPeer;
      dc: ReturnType<BrowserPeer["createDataChannel"]>;
    }> => {
      for (let attempt = 1; ; attempt++) {
        const pc = new Peer({ iceServers: [] });
        const dc = pc.createDataChannel("engine");
        try {
          await pc.setLocalDescription(await pc.createOffer());
          await until(
            () => pc.iceGatheringState === "complete",
            GATHER_MS,
          ).catch(() => {});
          const answer = await hub.signal("e1", pc.localDescription?.sdp ?? "");
          await pc.setRemoteDescription({ type: "answer", sdp: answer });
          return { pc, dc };
        } catch (e) {
          pc.close();
          if (attempt >= 3 || !/without ICE transport/.test(String(e))) throw e;
        }
      }
    };

    beforeEach(async () => {
      engineGot = [];
      engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
      engine.on("connection", (s) =>
        s.on("message", (d) => {
          engineGot.push(d.toString());
          s.send(JSON.stringify({ type: "echo", size: d.toString().length }));
        }),
      );
      await new Promise((r) => engine.once("listening", r));
      hub = await startHub();
      agent = make({
        registryUrl: hub.agentUrl,
        id: "e1",
        token: "t",
        enginePort: (engine.address() as AddressInfo).port,
        cacheDir: mkdtempSync(join(tmpdir(), "rtc-cache-")),
        log: () => {},
      });
      agent.start();
      while (!(await hub.has("e1")))
        await new Promise((r) => setTimeout(r, 20));
    });

    // Each handle may be unset: a beforeEach that failed must not be buried under this hook's TypeError.
    afterEach(async () => {
      agent?.stop();
      engine?.close();
      await hub?.stop();
    });

    it(
      "outlives a browser closing its path while the engine is talking",
      async () => {
        const { RTCPeerConnection } = (await import(
          "node-datachannel/polyfill"
        )) as unknown as {
          RTCPeerConnection: new (cfg: object) => BrowserPeer;
        };
        // The engine echoes every frame, so it is sending while the second browser hangs up.
        for (let round = 0; round < ROUNDS; round++) {
          const { pc, dc } = await offerPath(RTCPeerConnection);
          await until(() => dc.readyState === "open", OPEN_MS);
          for (const f of toFragments(JSON.stringify({ type: "ping" })))
            dc.send(f);
          await new Promise((r) => setTimeout(r, 200));
          pc.close();
          await new Promise((r) => setTimeout(r, 500));
        }
        expect(await hub.has("e1")).toBe(true);
      },
      ROUNDS * (GATHER_MS + OPEN_MS + 1000) + 10_000,
    );

    it("refuses an offer for an engine with no tunnel", async () => {
      await expect(hub.signal("nobody", "v=0")).rejects.toThrow(/no tunnel/);
    });
  },
);
