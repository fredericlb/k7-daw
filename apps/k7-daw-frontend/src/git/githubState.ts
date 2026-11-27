// githubState.ts — the rules of "Sign in with GitHub" as data: the device-flow screen's states, and how the picker orders,
// filters and dates a list of repositories. What a reader gets wrong: nothing here holds a token, the backend keeps it.

import {
  type Ago,
  arrange,
  type GithubRepo,
  pushedAgo,
} from "@studio/k7-wasm/git";
import type { ForgeStatus } from "./forgeState";

/** What `GET /git/github` says: GitHub's account under the forge seam. */
export type GithubStatus = ForgeStatus;

export type { GithubRepo };

export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  interval: number;
  expiresIn: number;
}

export interface PollAnswer {
  state: "pending" | "slow_down" | "expired" | "denied" | "ok";
  interval?: number;
  login?: string;
  keyNote?: string;
}

/** Settings → Services asks everything in one code: the person, the key, and `repo` for Connect's picker. */
export const GITHUB_SCOPES = ["read:user", "write:public_key", "repo"];

export const signedIn = (s: GithubStatus | null): boolean =>
  Boolean(s?.available && s.login);

export const hasRepoScope = (s: GithubStatus | null): boolean =>
  Boolean(s?.scopes.includes("repo"));

export type SignIn =
  | { phase: "starting" }
  | { phase: "waiting"; code: string; uri: string; interval: number }
  | { phase: "expired" }
  | { phase: "denied" }
  | { phase: "failed"; message: string }
  | { phase: "done"; login: string; keyNote?: string };

export type SignInEvent =
  | { type: "started"; code: DeviceCode }
  | { type: "polled"; answer: PollAnswer }
  | { type: "failed"; message: string }
  | { type: "again" };

export const startingSignIn: SignIn = { phase: "starting" };

/** The screen between events; an answer that arrives out of its phase changes nothing. */
export function signInAfter(s: SignIn, e: SignInEvent): SignIn {
  switch (e.type) {
    case "again":
      return startingSignIn;
    case "failed":
      return { phase: "failed", message: e.message };
    case "started":
      return {
        phase: "waiting",
        code: e.code.userCode,
        uri: e.code.verificationUri,
        interval: e.code.interval,
      };
    case "polled": {
      if (s.phase !== "waiting") return s;
      const a = e.answer;
      if (a.state === "expired") return { phase: "expired" };
      if (a.state === "denied") return { phase: "denied" };
      if (a.state === "ok")
        return { phase: "done", login: a.login ?? "", keyNote: a.keyNote };
      return { ...s, interval: a.interval ?? s.interval };
    }
  }
}

/** The repository's name without its owner. */
export const shortName = (fullName: string): string =>
  fullName.slice(fullName.indexOf("/") + 1);

export type { Ago };
export { arrange, pushedAgo };
