// Every service resolves on k7's origin — `docs.` read as `app.`, the `play.` and `k7.` aliases
// stripped, a former host read as its successor; on a loopback host that
// origin is the page's own, port and all.

import { afterEach, describe, expect, it } from "vitest";
import {
  isDocsHost,
  parsePorts,
  servicePorts,
  serviceUrl,
} from "./services.ts";

describe("serviceUrl", () => {
  it("echoes k7 back on whatever origin it's given, dev or prod", () => {
    expect(serviceUrl("https://app.k7.rocks/x", "k7")).toBe(
      "https://app.k7.rocks",
    );
    expect(serviceUrl("https://app.priv.k7.rocks/x", "k7")).toBe(
      "https://app.priv.k7.rocks",
    );
  });

  it("reads the docs host as the app beside it", () => {
    expect(serviceUrl("https://docs.k7.rocks/x", "k7")).toBe(
      "https://app.k7.rocks",
    );
    expect(serviceUrl("https://docs.priv.k7.rocks", "k7")).toBe(
      "https://app.priv.k7.rocks",
    );
  });

  it("reads a former host, alias or not, as the current origin", () => {
    expect(serviceUrl("https://st0.app/x", "k7")).toBe("https://app.k7.rocks");
    expect(serviceUrl("https://play.st0.app/songs/a", "k7")).toBe(
      "https://app.k7.rocks",
    );
    expect(serviceUrl("https://k7.st0.app/session/1", "k7")).toBe(
      "https://app.k7.rocks",
    );
    expect(serviceUrl("https://k7.priv.st0.app", "k7")).toBe(
      "https://app.priv.k7.rocks",
    );
    expect(serviceUrl("https://play.priv.st0.app", "k7")).toBe(
      "https://app.priv.k7.rocks",
    );
  });

  it("strips an alias label on a domain of its own, which needs no configuring", () => {
    expect(serviceUrl("https://play.studio.example/x", "k7")).toBe(
      "https://studio.example",
    );
  });

  it("never strips a label down to a bare TLD", () => {
    expect(serviceUrl("https://k7.rocks", "k7")).toBe("https://k7.rocks");
  });

  it("returns '' for a string that is not a URL, never a throw", () => {
    expect(serviceUrl("not a url", "k7")).toBe("");
  });

  it("keeps a loopback page's own origin, port and all, for every service", () => {
    expect(serviceUrl("http://127.0.0.1:8789", "k7")).toBe(
      "http://127.0.0.1:8789",
    );
    expect(serviceUrl("http://localhost:5223", "k7")).toBe(
      "http://localhost:5223",
    );
    expect(serviceUrl("http://localhost:5223", "k7")).toBe(
      "http://localhost:5223",
    );
  });
});

describe("isDocsHost", () => {
  it("names the documentation's host", () => {
    expect(isDocsHost("docs.k7.rocks")).toBe(true);
    expect(isDocsHost("docs.priv.k7.rocks")).toBe(true);
    expect(isDocsHost("app.k7.rocks")).toBe(false);
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
