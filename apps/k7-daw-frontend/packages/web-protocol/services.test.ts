// Every service resolves on the origin it is given: the host and port of a page are k7's own.

import { describe, expect, it } from "vitest";
import { serviceUrl } from "./services.ts";

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
