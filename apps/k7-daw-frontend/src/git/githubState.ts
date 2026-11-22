// githubState.ts — the rules of "Sign in with GitHub" as data: the device-flow screen's states, and how the picker orders,
// filters and dates a list of repositories. What a reader gets wrong: nothing here holds a token, the backend keeps it.

import type { ForgeStatus } from "./forgeState";

/** What `GET /git/github` says: GitHub's account under the forge seam. */
export type GithubStatus = ForgeStatus;

export interface GithubRepo {
  fullName: string;
  sshUrl: string;
  private: boolean;
  pushedAt: string | null;
  /** Carries the `k7-workspace` topic. */
  k7: boolean;
}

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

/** Connect and Clone show GitHub's picker only to a person signed in with `repo`; nobody else sees GitHub there. */
export const canPick = (s: GithubStatus | null): boolean =>
  signedIn(s) && hasRepoScope(s);

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

/** The picker's two lists: k7 workspaces under their own heading, then the rest, each most recently pushed first. */
export function arrange(
  repos: GithubRepo[],
  filter: string,
): { k7: GithubRepo[]; others: GithubRepo[] } {
  const q = filter.trim().toLowerCase();
  const shown = repos.filter((r) => r.fullName.toLowerCase().includes(q));
  const byPush = (a: GithubRepo, b: GithubRepo) =>
    Date.parse(b.pushedAt ?? "") - Date.parse(a.pushedAt ?? "") || 0;
  const sorted = [...shown].sort(byPush);
  return {
    k7: sorted.filter((r) => r.k7),
    others: sorted.filter((r) => !r.k7),
  };
}

export type Ago = {
  unit: "now" | "minutes" | "hours" | "days" | "months" | "years";
  n: number;
};

/** How long ago a push was, in the largest whole unit. */
export function pushedAgo(iso: string | null, now: number): Ago {
  const t = Date.parse(iso ?? "");
  if (Number.isNaN(t)) return { unit: "years", n: 99 };
  const min = Math.max(0, Math.floor((now - t) / 60_000));
  if (min < 1) return { unit: "now", n: 0 };
  if (min < 60) return { unit: "minutes", n: min };
  if (min < 60 * 24) return { unit: "hours", n: Math.floor(min / 60) };
  const days = Math.floor(min / (60 * 24));
  if (days < 30) return { unit: "days", n: days };
  if (days < 365) return { unit: "months", n: Math.floor(days / 30) };
  return { unit: "years", n: Math.floor(days / 365) };
}
