import { describe, expect, it } from "vitest";
import { relayWsUrl } from "./probe.ts";

describe("relayWsUrl", () => {
  it("reaches a relay on ANOTHER origin at that origin, not the page's", () => {
    const r = relayWsUrl(
      "https://app.priv.k7.rocks",
      "05ff",
      "t.k",
      "https://app.priv.k7.rocks/@local/ws/p.k7",
    );
    expect(r).toBe("wss://app.priv.k7.rocks/api/engine/05ff/ws?ticket=t.k");
  });

  it("reaches a relay given as a PATH on the page's own origin", () => {
    const r = relayWsUrl(
      "/pl8",
      "e1",
      "x",
      "https://studio.frediver.se/songs/a.k7",
    );
    expect(r).toBe("wss://studio.frediver.se/pl8/api/engine/e1/ws?ticket=x");
  });

  it("dials a plain-HTTP relay over ws:", () => {
    const r = relayWsUrl(
      "http://localhost:8789",
      "e1",
      "x",
      "http://localhost:5223/",
    );
    expect(r).toBe("ws://localhost:8789/api/engine/e1/ws?ticket=x");
  });
});
