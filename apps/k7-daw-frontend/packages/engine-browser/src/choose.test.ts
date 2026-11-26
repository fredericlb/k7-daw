// The page's half of the engine-choice policy: what it reads off a probed engine and hands to Rust.
// The policy's own cases are song-schema's tests (engine/choose.rs); these pin the address facts, which need a URL parse.

import type { PublicEngine } from "@studio/web-protocol";
import { describe, expect, it } from "vitest";
import { bestEngine, chooseEngine } from "./choose.ts";
import { type RankedEngine, reachedDirectly } from "./probe.ts";

const engine = (id: string, ms: number | null): RankedEngine<PublicEngine> =>
  ({
    engine: {
      id,
      hostname: id,
      addresses: {
        loopback: "127.0.0.1",
        lanIps: ["192.168.1.20"],
        port: 8765,
      },
      assigned: false,
      sameSourceIp: false,
    },
    latency:
      ms === null
        ? null
        : { networkMs: ms, audioMs: 0, totalMs: ms, quality: "Q1" },
    wsUrl: ms === null ? null : "ws://192.168.1.20:8765",
    fileBase: ms === null ? null : `http://${id}:8770`,
    token: ms === null ? null : "t",
  }) as unknown as RankedEngine<PublicEngine>;

const rated = (
  id: string,
  ms: number,
  quality: "Q1" | "Q2" | "Q3",
  score?: number,
): RankedEngine<PublicEngine> => {
  const r = engine(id, ms);
  return {
    ...r,
    latency: r.latency && { ...r.latency, quality },
    engine: {
      ...r.engine,
      performance: score === undefined ? undefined : { score },
    },
  } as unknown as RankedEngine<PublicEngine>;
};

/** The same engine, reached through the origin's relay rather than on its own address. */
const relayed = (r: RankedEngine<PublicEngine>): RankedEngine<PublicEngine> =>
  ({
    ...r,
    wsUrl: `wss://app.k7.rocks/api/engine/${r.engine.id}/ws?ticket=x`,
  }) as RankedEngine<PublicEngine>;

describe("chooseEngine", () => {
  it("takes the remembered one, and the only reachable one", () => {
    const two = [engine("a", 5), engine("b", 30)];
    expect(chooseEngine(two, "b").engine?.engine.id).toBe("b");
    expect(chooseEngine([engine("a", 5)], undefined).engine?.engine.id).toBe(
      "a",
    );
    expect(chooseEngine(two, undefined)).toEqual({
      engine: null,
      ambiguous: true,
    });
  });

  it("never auto-picks an engine reached only through the relay", () => {
    expect(chooseEngine([relayed(engine("evil", 2))], undefined).engine).toBe(
      null,
    );
    expect(chooseEngine([relayed(engine("mine", 9))], "mine").engine).not.toBe(
      null,
    );
  });
});

describe("bestEngine", () => {
  it("reads the engine's rating, this machine and the compat callback", () => {
    const here = {
      ...rated("mac", 12, "Q1", 7000),
      wsUrl: "ws://127.0.0.1:8765",
    } as RankedEngine<PublicEngine>;
    const pc = rated("studio-pc", 8, "Q1", 12000);
    expect(bestEngine([pc, here])?.engine.id).toBe("mac");
    expect(bestEngine([pc, here], { remembered: "studio-pc" })?.engine.id).toBe(
      "studio-pc",
    );
    expect(
      bestEngine([pc, here], { compat: (id) => (id === "mac" ? 60 : 100) })
        ?.engine.id,
    ).toBe("studio-pc");
    expect(bestEngine([pc, here], { compat: () => null })?.engine.id).toBe(
      "mac",
    );
  });

  it("never takes an engine reached only through the relay", () => {
    expect(bestEngine([relayed(rated("evil", 2, "Q1", 99999))])).toBeNull();
    expect(
      bestEngine([
        relayed(rated("evil", 1, "Q1", 99999)),
        rated("lan", 30, "Q2", 500),
      ])?.engine.id,
    ).toBe("lan");
  });
});

// A lookalike may advertise any address: only one of its own private addresses, dialled directly, counts.
describe("who is reached directly", () => {
  it("does not count a public address as direct", () => {
    const pub = rated("pub", 2, "Q1");
    const at = {
      ...pub,
      engine: {
        ...pub.engine,
        addresses: {
          loopback: "127.0.0.1",
          lanIps: ["203.0.113.9"],
          port: 8765,
        },
      },
      wsUrl: "ws://203.0.113.9:8765",
    } as RankedEngine<PublicEngine>;
    expect(reachedDirectly(at)).toBe(false);
    expect(reachedDirectly(rated("lan", 2, "Q1"))).toBe(true);
  });
});
