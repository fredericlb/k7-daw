// testAgents.ts — the agents the tunnel suites run against: this package's own, and the native
// supervisor's (apps/k7-daw-backend/packages/k7-supervisor), whose `tunnel_harness` answers the same test jobs.
// Two implementations of one wire, held to one set of tests. The native one is skipped until built.

import { type ChildProcess, spawn } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { TunnelAgent, type TunnelAgentOptions } from "./agent.ts";

export interface AgentHandle {
  start(): void;
  stop(): void;
}

export interface AgentKind {
  name: string;
  /** Whether it answers a WebRTC offer: the native supervisor's refuses, the browser stays on the tunnel. */
  rtc: boolean;
  make(opts: TunnelAgentOptions): AgentHandle;
}

/** The harness `$TUNNEL_HARNESS_BIN` names, else the newest built under the backend's cargo target, or null. */
function harness(): string | null {
  const named = process.env.TUNNEL_HARNESS_BIN;
  if (named) return existsSync(named) ? named : null;
  const target = join(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
    "k7-daw-backend",
    "target",
  );
  const found: string[] = [];
  for (const profile of ["debug", "release"])
    for (const name of ["tunnel_harness", "tunnel_harness.exe"]) {
      const p = join(target, profile, name);
      if (existsSync(p)) found.push(p);
    }
  found.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return found[0] ?? null;
}

const node: AgentKind = {
  name: "node",
  rtc: true,
  make: (opts) => new TunnelAgent(opts),
};

/** The harness knows its jobs by heart, so `opts.jobs` is not passed — the suites use its names. */
function native(bin: string): AgentKind {
  return {
    name: "native",
    rtc: false,
    make: (opts) => {
      let child: ChildProcess | null = null;
      return {
        start() {
          child = spawn(
            bin,
            [
              opts.registryUrl,
              opts.id,
              opts.token,
              String(opts.enginePort),
              opts.cacheDir,
            ],
            { stdio: ["pipe", "ignore", "inherit"] },
          );
        },
        stop() {
          child?.stdin?.end();
          child?.kill();
          child = null;
        },
      };
    },
  };
}

const bin = harness();
export const agentKinds: AgentKind[] = bin ? [node, native(bin)] : [node];
