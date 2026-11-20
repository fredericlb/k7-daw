// engineChains.ts — what an ENGINE separates with, kept in that machine's own `engine-host.json` and reached over
// its tunnel (`apps/k7/backend-rs/docs/engine/chains.md`). Only its owner may write: the server answers 403 for
// anyone else, and an engine holding no tunnel answers 503 rather than a copy of an answer it cannot vouch for.

import { API_ROOT } from "../api";

/** The two a machine answers by `want`; absent is "this machine names none". */
export interface EngineChains {
  stems?: string;
  instrumental?: string;
}

/** The chain an engine's machine holds, read from the machine itself. */
export async function loadChains(engineId: string): Promise<EngineChains> {
  return ask(engineId, "GET");
}

/** Name one on that machine: a key left out is untouched, and `""` unsays it. The answer is what it then holds. */
export async function saveChains(
  engineId: string,
  set: EngineChains,
): Promise<EngineChains> {
  return ask(engineId, "PUT", set);
}

async function ask(
  engineId: string,
  method: "GET" | "PUT",
  json?: EngineChains,
): Promise<EngineChains> {
  const r = await fetch(
    `${API_ROOT}/engines/${encodeURIComponent(engineId)}/chains`,
    {
      method,
      credentials: "include",
      headers: json ? { "content-type": "application/json" } : undefined,
      body: json ? JSON.stringify(json) : undefined,
    },
  );
  const body = (await r.json().catch(() => null)) as {
    chains?: EngineChains;
    error?: string;
  } | null;
  if (!r.ok)
    throw new Error(body?.error || `the engine did not answer (${r.status})`);
  return body?.chains ?? {};
}
