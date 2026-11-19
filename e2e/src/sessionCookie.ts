// sessionCookie.ts — the signed session cookie the front reads, minted here for a suite that signs no one in.
// What a reader gets wrong: the format is the front's (`k7-auth`), frozen by apps/k7/backend-rs/fixtures/session-vectors.json.

import Keygrip from "keygrip";

/** The principal in the session cookie; the index signature carries the account's own fields. */
export interface SessionUser {
  sub?: string;
  name?: string;
  email?: string | null;
  groups?: string[];
  [key: string]: unknown;
}

/** How long a session lives from its sign-in. */
const SESSION_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

/** The Cookie header the front would issue for `user`, stamps included. */
export function sessionCookieHeader(
  user: SessionUser,
  opts: { cookieName: string; secret: string; iat?: number },
): string {
  const iat = opts.iat ?? Date.now();
  const payload = { user, iat, exp: iat + SESSION_MAX_AGE_MS };
  const value = Buffer.from(JSON.stringify(payload)).toString("base64");
  const sig = new Keygrip([opts.secret]).sign(`${opts.cookieName}=${value}`);
  return `${opts.cookieName}=${value}; ${opts.cookieName}.sig=${sig}`;
}
