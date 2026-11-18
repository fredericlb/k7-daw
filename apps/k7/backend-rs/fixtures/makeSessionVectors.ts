// makeSessionVectors.ts — writes session-vectors.json from @studio/oidc's own sessionCookieHeader, the source of truth.
// Run with `npx tsx apps/k7/backend-rs/fixtures/makeSessionVectors.ts`; both sides' tests then check it (apps/k7/backend-rs/docs/sessions.md).
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { type SessionUser, sessionCookieHeader } from "@studio/oidc";

const cases: { secret: string; iat: number; user: SessionUser }[] = [
  {
    secret: "dev-secret",
    iat: 1_700_000_000_000,
    user: { sub: "e2e", name: "E2E", email: "e2e@st0.app", groups: [] },
  },
  {
    secret: "s3cr3t/with+chars=",
    iat: 1_761_000_000_123,
    user: {
      sub: "abc|123",
      name: "Frédéric Ünïcode 🎛",
      email: null,
      groups: ["studio", "admin"],
    },
  },
  {
    secret: "x",
    iat: 1_000,
    user: {
      sub: "q",
      name: 'quote " back \\ slash\ttab\u0001ctl',
      email: "a@b.c",
      groups: [],
      extra: { nested: [1, 2.5, true, null] },
    },
  },
  {
    secret: "long".repeat(40),
    iat: 1_761_999_999_999,
    user: {
      sub: "s".repeat(300),
      name: "<script>&amp;</script>",
      email: "x@y.z",
    },
  },
];

const vectors = cases.map(({ secret, iat, user }) => {
  const header = sessionCookieHeader(user, {
    cookieName: "studio.sid",
    secret,
    iat,
  });
  const [valuePair, sigPair] = header.split("; ");
  const value = valuePair.slice(valuePair.indexOf("=") + 1);
  const sig = sigPair.slice(sigPair.indexOf("=") + 1);
  return {
    cookieName: "studio.sid",
    secret,
    iat,
    user,
    json: Buffer.from(value, "base64").toString("utf8"),
    value,
    sig,
    header,
  };
});

writeFileSync(
  join(import.meta.dirname, "session-vectors.json"),
  `${JSON.stringify(vectors, null, 2)}\n`,
);
