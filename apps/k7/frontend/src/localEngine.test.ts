import { describe, expect, it } from "vitest";
import { localEngineOf } from "./localEngine";
import type { RankedEngine } from "./types";

const row = (
  id: string,
  wsUrl: string | null,
  token: string | null,
  access: "owner" | "unowned" = "owner",
  loopbackPort = 0,
) =>
  ({
    engine: {
      id,
      access,
      addresses: { loopback: "127.0.0.1", lanIps: [], port: loopbackPort },
    },
    wsUrl,
    token,
  }) as unknown as RankedEngine;

describe("localEngineOf", () => {
  it("takes the engine of this device, never one elsewhere", () => {
    const ranked = [
      row("far", "wss://studio.example/relay/x", "t"),
      row("here", "ws://127.0.0.1:18789/relay/x", "t"),
    ];
    expect(localEngineOf(ranked)?.engine.id).toBe("here");
    expect(localEngineOf(ranked.slice(0, 1))).toBeNull();
  });

  it("never takes an unowned stranger's engine", () => {
    const ranked = [
      row("stranger", "ws://127.0.0.1:18789/relay/x", "t", "unowned"),
    ];
    expect(localEngineOf(ranked)).toBeNull();
  });

  it("prefers the engine reached directly over a relayed one", () => {
    const ranked = [
      row("relayed", "ws://127.0.0.1:18789/relay/x", "t", "owner", 8765),
      row("direct", "ws://127.0.0.1:8765", "t", "owner", 8765),
    ];
    expect(localEngineOf(ranked)?.engine.id).toBe("direct");
  });

  it("prefers a local engine that answered", () => {
    const ranked = [
      row("silent", "ws://localhost:1/engine", null),
      row("up", "ws://localhost:2/engine", "t"),
    ];
    expect(localEngineOf(ranked)?.engine.id).toBe("up");
    expect(localEngineOf(ranked.slice(0, 1))?.engine.id).toBe("silent");
  });
});
