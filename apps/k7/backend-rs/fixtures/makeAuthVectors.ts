// makeAuthVectors.ts — writes auth-vectors.json: library tokens minted by @studio/oidc's createApiTokens.
// Run with `npx tsx apps/k7/backend-rs/fixtures/makeAuthVectors.ts`; both sides' tests then check it (apps/k7/backend-rs/docs/sessions.md).
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { createApiTokens, memoryTokenStore } from "@studio/oidc";

// A century: the vectors must not expire under the tests that read them.
const TTL_MS = 100 * 365 * 24 * 60 * 60 * 1000;

const tokenCases = [
  { secret: "dev-secret:pl8", sub: "e2e" },
  { secret: "s3cr3t/with+chars=:pl8", sub: "abc|123 Frédéric 🎛" },
  { secret: ":pl8", sub: 'quote " back \\ slash' },
];

const tokens = tokenCases.map(({ secret, sub }) => {
  const store = memoryTokenStore();
  const token = createApiTokens(secret, { store, ttlMs: TTL_MS }).issue(sub, {
    label: "vector",
  });
  const payload = JSON.parse(
    Buffer.from(token.split(".")[0], "base64url").toString("utf8"),
  );
  return { secret, sub, token, payload, record: store.list(sub)[0] };
});

const file = join(import.meta.dirname, "auth-vectors.json");
writeFileSync(file, `${JSON.stringify({ tokens }, null, 2)}\n`);
