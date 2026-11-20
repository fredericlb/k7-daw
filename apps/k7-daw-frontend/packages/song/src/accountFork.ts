// accountFork.ts — FORK a module of the origin's libraries into the account's own: a module at
// `@account/<path>` that `extends` it, its panel copied so it draws the same, and the project's refs
// repointed to it. The library is never written: changing a module of it is forking it, and the
// fork keeps inheriting whatever it does not change. See ../../../../../docs/library-versions-plan.md.

import { stub } from "./editStub";

/** What a fork did: the new ref, its folder, and the project files that now name it. */
export interface AccountFork {
  ref: string;
  folder: string;
  created: boolean;
  repointed: string[];
}

/** Fork the library module `ref` (as `projectDir`'s workspace pins it) into `@account`, and repoint the project to it. */
export function forkIntoAccount(projectDir: string, ref: string): AccountFork {
  return stub("song/accountFork.forkIntoAccount", projectDir, ref);
}
