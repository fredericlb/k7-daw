// The browser half of the bench's session — the file service's `session_of` is the other, pinned to the same SESSIONS.bench by its Rust test.

import { SESSIONS, sessionFor } from "@studio/engine-protocol/sessions";
import { describe, expect, it } from "vitest";
import { engineSession } from "./engineSession";

describe("engineSession", () => {
  it("plays the bench in its own session, and a song in k7's", () => {
    expect(engineSession("a@x.org", "/@banc/vocals/warm/live.k7", "/")).toBe(
      sessionFor(SESSIONS.bench, "a@x.org"),
    );
    expect(engineSession("a@x.org", "/@local/ws/song.k7/graph", "/")).toBe(
      sessionFor(SESSIONS.k7, "a@x.org"),
    );
    expect(engineSession("a@x.org", "/", "/")).toBe(
      sessionFor(SESSIONS.k7, "a@x.org"),
    );
  });
});
