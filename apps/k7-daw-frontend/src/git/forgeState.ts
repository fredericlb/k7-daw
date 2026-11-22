// forgeState.ts — the accounts of Settings → Services as data: what `GET /git/forges` lists, one entry per forge account.
// What a reader gets wrong: the page still calls GitHub through `/git/github/*`, an alias of `/git/forges/github.com/*`.

/** The forges k7 knows how to draw; a new one is a new member here. */
export type ForgeKind = "github";

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
}
