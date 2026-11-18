import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  type ApiTokenRecord,
  createApiTokens,
  memoryTokenStore,
} from "@studio/oidc";
import { describe, expect, it } from "vitest";

// The tokens the Rust front must mint byte for byte (apps/k7/backend-rs/docs/sessions.md).
const file = join(
  import.meta.dirname,
  "../../backend-rs/fixtures/auth-vectors.json",
);
const vectors: {
  tokens: {
    secret: string;
    sub: string;
    token: string;
    record: ApiTokenRecord;
  }[];
} = JSON.parse(readFileSync(file, "utf8"));

describe("auth vectors", () => {
  it("are tokens @studio/oidc still accepts", () => {
    for (const v of vectors.tokens) {
      const store = memoryTokenStore();
      store.add(v.record);
      expect(createApiTokens(v.secret, { store }).verify(v.token)).toBe(v.sub);
    }
  });
});
