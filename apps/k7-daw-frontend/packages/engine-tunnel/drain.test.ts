// drain.test.ts — the unsent-bytes mark a tunnel sender waits at, and that the supervisor's Rust
// sender stops at the same one. See docs/tunnel.md.

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { drained, HIGH_WATER_BYTES } from "./drain.ts";

describe("drained", () => {
  it("lets a sender through at the mark and holds it above until the buffer drains", async () => {
    const sock = { bufferedAmount: HIGH_WATER_BYTES, readyState: 1 };
    await drained(sock);
    sock.bufferedAmount = HIGH_WATER_BYTES + 1;
    let done = false;
    const wait = drained(sock).then(() => {
      done = true;
    });
    await new Promise((r) => setTimeout(r, 20));
    expect(done).toBe(false);
    sock.bufferedAmount = 0;
    await wait;
    expect(done).toBe(true);
  });

  it("stops waiting on a socket that closed", async () => {
    await drained({ bufferedAmount: HIGH_WATER_BYTES * 2, readyState: 3 });
  });

  it("is the supervisor's HIGH_WATER_BYTES too, so both ends stop at one megabyte", () => {
    const source = readFileSync(
      new URL(
        "../../../k7-daw-backend/packages/k7-supervisor/src/tunnel/wire.rs",
        import.meta.url,
      ),
      "utf8",
    );
    const m = /HIGH_WATER_BYTES: usize\s*=\s*([\d\s*]+);/.exec(source);
    const rust = (m?.[1] ?? "0")
      .split("*")
      .reduce((a, x) => a * Number(x.trim()), 1);
    expect(HIGH_WATER_BYTES).toBe(1024 * 1024);
    expect(rust).toBe(HIGH_WATER_BYTES);
  });
});
