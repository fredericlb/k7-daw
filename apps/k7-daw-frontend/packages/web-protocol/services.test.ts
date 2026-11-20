// Every service resolves on the origin it is given: the host and port of a page are k7's own.

import { afterEach, describe, expect, it } from "vitest";
import { parsePorts, servicePorts, serviceUrl } from "./services.ts";

describe("serviceUrl", () => {
  it("echoes the origin it is given, path dropped", () => {
    expect(serviceUrl("https://app.k7.rocks/x", "k7")).toBe(
      "https://app.k7.rocks",
    );
    expect(serviceUrl("https://studio.example:8443/a?b#c", "k7")).toBe(
      "https://studio.example:8443",
    );
  });

  it("returns '' for a string that is not a URL, never a throw", () => {
    expect(serviceUrl("not a url", "k7")).toBe("");
  });

  it("keeps a loopback page's own origin, port and all", () => {
    expect(serviceUrl("http://127.0.0.1:8789", "k7")).toBe(
      "http://127.0.0.1:8789",
    );
    expect(serviceUrl("http://localhost:5223", "k7")).toBe(
      "http://localhost:5223",
    );
  });
});

describe("moved loopback ports", () => {
  afterEach(() => {
    delete process.env.STUDIO_PORTS;
    delete (globalThis as Record<string, unknown>).__STUDIO_PORTS__;
  });

  it("reads a host's table from the environment, for node", () => {
    process.env.STUDIO_PORTS = "k7=18789";
    expect(servicePorts().k7).toBe(18789);
  });

  it("prefers the page's global, which a preload sets", () => {
    process.env.STUDIO_PORTS = "k7=1";
    (globalThis as Record<string, unknown>).__STUDIO_PORTS__ = "k7=18789";
    expect(servicePorts().k7).toBe(18789);
  });

  it("drops what it cannot read and keeps the rest of the table", () => {
    expect(parsePorts("pl8=18795, nope=1, k7=70000, k7=18789")).toEqual({
      k7: 18789,
    });
    process.env.STUDIO_PORTS = "pl8=18787";
    expect(servicePorts().k7).toBe(8789);
  });
});
