// makeNumberVectors.ts — writes number-vectors.json from node's own toFixed, String, Math.round and textHash.
// Run with `npx tsx apps/k7/backend-rs/fixtures/makeNumberVectors.ts`; k7-js's tests/all/number.rs checks it (docs/parity.md).
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { numberVectors } from "./numberVectors.ts";

// `makeNumberVectors.ts <batches> <file>` writes a larger set elsewhere, for a local run of the ignored test.
const [batches, out] = process.argv.slice(2);
writeFileSync(
  out ?? join(import.meta.dirname, "number-vectors.json"),
  `${JSON.stringify(numberVectors(batches ? Number(batches) : undefined), null, 2)}\n`,
);
