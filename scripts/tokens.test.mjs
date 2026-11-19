// tokens.test.mjs — `npm run tokens` gives the front its own API_TOKEN_SECRET in env.yaml; each case runs the real
// script on a scratch copy of env.example.yaml. Run: node --test scripts/tokens.test.mjs

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, beforeEach, describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const script = join(repo, "scripts", "tokens.mjs");

let home;
let envFile;

/** The script against a scratch copy of env.yaml, so it writes nothing of the developer's. */
const run = (...args) =>
  execFileSync(process.execPath, [script, "--env", envFile, ...args], {
    cwd: repo,
    encoding: "utf8",
  });

const secretIn = (file) =>
  parse(readFileSync(file, "utf8"))?.["k7-front"]?.API_TOKEN_SECRET;

beforeEach(() => {
  home = mkdtempSync(join(tmpdir(), "studio-tokens-"));
  envFile = join(home, "env.yaml");
  copyFileSync(join(repo, "env.example.yaml"), envFile);
});
afterEach(() => rmSync(home, { recursive: true, force: true }));

describe("the tokens script", () => {
  it("generates the signing secret when the service has none", () => {
    assert.equal(secretIn(join(repo, "env.example.yaml")), "");
    run();
    assert.match(secretIn(envFile), /^[0-9a-f]{64}$/);
  });

  it("leaves a secret already there alone", () => {
    run();
    const first = secretIn(envFile);
    assert.match(run(), /already set/);
    assert.equal(secretIn(envFile), first);
  });

  it("writes nothing on --dry-run", () => {
    const before = readFileSync(envFile, "utf8");
    run("--dry-run");
    assert.equal(readFileSync(envFile, "utf8"), before);
  });

  it("keeps env.yaml's comments", () => {
    run();
    assert.match(readFileSync(envFile, "utf8"), /# /);
  });
});
