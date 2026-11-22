// forgeState.ts — the accounts of Settings → Services as data: what `GET /git/forges` lists, one entry per forge account.
// What a reader gets wrong: GitHub's page still calls `/git/github/*`, an alias of `/git/forges/github.com/*`.

/** The forges k7 knows how to draw; a new one is a new member here. */
export type ForgeKind = "github" | "forgejo";

/** What `GET /git/forges/<id>` says: the sign-in exists in this build, and who is signed in. */
export interface ForgeStatus {
  available: boolean;
  login?: string;
  scopes: string[];
}

/** One entry of `GET /git/forges`: the account's id (its forge's host), its kind and its name. */
export interface ForgeAccount extends ForgeStatus {
  id: string;
  kind: ForgeKind;
  name: string;
  /** A Forgejo server's address, as the person typed it. */
  url?: string;
}

/** What `POST /git/forges/probe` read from a server without a token. */
export interface ForgeProbe {
  id: string;
  url: string;
  version: string;
  pageSize: number;
  lfs: boolean;
  rootUrl: string;
  rootAnswers: boolean;
  redirects: string[];
  scopes: string[];
}

/** What `POST /git/forges/<id>/start` answers: the page to open in a browser. */
export interface ForgeStart {
  authorizeUrl: string;
  redirect: string;
  expiresIn: number;
}

/** How a Forgejo sign-in ended, said once by `POST …/poll`. */
export interface ForgePoll {
  state: "pending" | "ok" | "denied" | "error" | "expired";
  login?: string;
  scopes?: string[];
  keyNote?: string;
  error?: string;
}

export const forgejoServers = (a: ForgeAccount[]): ForgeAccount[] =>
  a.filter((x) => x.kind === "forgejo");

/** Whether an account can list repositories: GitHub needs `repo`, a Forgejo server asked its scopes with the sign-in. */
export const canPickFrom = (a: ForgeAccount): boolean =>
  a.available &&
  Boolean(a.login) &&
  (a.kind === "forgejo" || a.scopes.includes("repo"));

/** A refusal that says the sign-in is gone and must be done again, for either forge. */
export const isSignout = (code: string): boolean => code.endsWith("-signout");

/** Servers whose sign-in the front dropped while this page ran; Settings then says "Sign in again". */
export const droppedSignIns = new Set<string>();

/** What a screen calls an account: GitHub by its name, a server by its host (the routes name every Forgejo "Forgejo"). */
export const accountLabel = (a: ForgeAccount): string =>
  a.kind === "forgejo" ? a.id : a.name;
