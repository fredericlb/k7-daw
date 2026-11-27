// gitTypes.ts — the shapes the git routes answer (apps/k7-daw-backend/docs/files/git.md), generated from the Rust structs.
// What a reader gets wrong: nothing here is written by hand but the unions the routes spell as literals and the page's names.

import type { Conflict } from "@studio/k7-wasm/git";

export type {
  CommitInfo,
  Conflict,
  GitState,
  MergeChoice as Choice,
  PullState,
  StatusEntry,
} from "@studio/k7-wasm/git";

export type MergeView =
  | { pending: false }
  | { pending: true; theirs: string; conflicts: Conflict[] };
