import { describe, expect, it } from "vitest";
import { localEngineOf } from "./localEngine";
import type { RankedEngine } from "./types";

const row = (id: string, wsUrl: string | null, token: string | null) =>
  ({ engine: { id }, wsUrl, token }) as unknown as RankedEngine;

describe("localEngineOf", () => {
  it("takes the engine on this machine, never one elsewhere", () => {
    const ranked = [
      row("far", "wss://studio.example/relay/x", "t"),
      row("here", "ws://127.0.0.1:18789/engine", "t"),
    ];
    expect(localEngineOf(ranked)?.engine.id).toBe("here");
    expect(localEngineOf(ranked.slice(0, 1))).toBeNull();
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
