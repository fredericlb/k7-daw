// shims.test.ts — the Worker's builtin stand-ins agree with node's.

import { createHash as nodeHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { createHash, randomUUID } from "./crypto.ts";

describe("the crypto shim", () => {
  const sizes = [0, 1, 3, 55, 56, 57, 63, 64, 65, 119, 120, 1000, 100_000];
  for (const alg of ["sha1", "sha256"]) {
    it(`${alg} digests like node at every padding boundary`, () => {
      for (const n of sizes) {
        const data = new Uint8Array(n).map((_, i) => (i * 31 + n) & 255);
        for (const enc of ["hex", "base64"] as const)
          expect(createHash(alg).update(data).digest(enc)).toBe(
            nodeHash(alg).update(data).digest(enc),
          );
      }
    });
  }

  it("chains updates of strings and bytes, accents included", () => {
    const mine = createHash("sha256")
      .update("été\0")
      .update(new Uint8Array([1, 2, 3]))
      .digest("hex");
    const theirs = nodeHash("sha256")
      .update("été\0")
      .update(new Uint8Array([1, 2, 3]))
      .digest("hex");
    expect(mine).toBe(theirs);
  });

  it("refuses an input or output encoding it does not hold, and makes a uuid without a secure context", () => {
    expect(() => createHash("sha1").update("ab", "hex" as never)).toThrow(
      /not supported/,
    );
    expect(() =>
      createHash("sha1")
        .update("a")
        .digest("latin1" as never),
    ).toThrow(/not supported/);
    const real = crypto.randomUUID;
    Object.defineProperty(crypto, "randomUUID", {
      value: undefined,
      configurable: true,
    });
    try {
      expect(randomUUID()).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      );
    } finally {
      Object.defineProperty(crypto, "randomUUID", {
        value: real,
        configurable: true,
      });
    }
  });

  it("refuses an algorithm it does not hold, and makes uuids", () => {
    expect(() => createHash("md5")).toThrow(/not supported/);
    expect(randomUUID()).toMatch(/^[0-9a-f-]{36}$/);
  });
});
