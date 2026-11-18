// owners.ts — who holds a claim, and what an account is to an engine, as the TypeScript that stays reads the
// registry's answers. The owners themselves are the Rust front's (apps/k7/backend-rs/docs/engine.md).

/** One engine's owner and shares, as the registry keeps them in `engine-owners.json`. */
export interface Ownership {
  /** The account its heartbeat proves, spelled as `claimHolder` spells the session. */
  owner: string;
  /** The accounts it is shared with, lowercase emails. */
  shared: string[];
  /** Shared with every signed-in account of this server, present and future. */
  everyone: boolean;
  /** Epoch ms this owner was assigned. */
  since: number;
}

/** What an account is to one engine: only the first three may claim it. */
export type EngineRole = "owner" | "shared" | "server" | "unowned" | "none";

/** The account a claim is held under, from the session — the one name every claim and owner check compares against. */
export function claimHolder(req: {
  user?: { email?: string | null; sub?: string | null };
}): string {
  return req.user?.email || req.user?.sub || "";
}
