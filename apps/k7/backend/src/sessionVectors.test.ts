import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type SessionUser, sessionCookieHeader } from "@studio/oidc";
import { describe, expect, it } from "vitest";

// The cookies the Rust front must read and mint byte for byte (apps/k7/backend-rs/docs/sessions.md).
const file = join(
  import.meta.dirname,
  "../../backend-rs/fixtures/session-vectors.json",
);
const vectors: {
  cookieName: string;
  secret: string;
  iat: number;
  user: SessionUser;
  header: string;
}[] = JSON.parse(readFileSync(file, "utf8"));

describe("session vectors", () => {
  it("are what @studio/oidc mints today", () => {
    for (const v of vectors) {
      const header = sessionCookieHeader(v.user, {
        cookieName: v.cookieName,
        secret: v.secret,
        iat: v.iat,
      });
      expect(header).toBe(v.header);
    }
  });
});
