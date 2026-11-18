// session.ts — a signed-in browser for the suite, without the identity provider.
// The cookie is signed with the instance's own SESSION_SECRET, read from this machine's env.yaml:
// it only works against an instance whose secret is on disk here, which is the point (docs/e2e.md).
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { loadServiceEnv } from "@studio/env";
import { type SessionUser, sessionCookieHeader } from "@studio/oidc";

const COOKIE_NAME = "studio.sid";
// The backend's env.yaml section, the one whose SESSION_SECRET signs its cookies.
const SERVICE = "k7-backend";

/** True when env.yaml starts the front as a device (`STUDIO_LOCAL_USER`), where every session is the one local user. */
export function deviceBed(): boolean {
  try {
    return backendEnv().STUDIO_LOCAL_USER === "1";
  } catch {
    return false;
  }
}

/** The account a spec's `user` is: itself on a server, the one local user on a device bed. */
export const acct = (user: SessionUser): SessionUser =>
  deviceBed() ? E2E_USER : user;

/** A dedicated account on a server; on a device bed the local user itself, the only account a device has (e2e/docs/e2e.md). */
export const E2E_USER: SessionUser = deviceBed()
  ? ({
      sub: "dev-user",
      name: "Local user",
      email: "dev@example.com",
      groups: [],
      local: true,
    } as SessionUser)
  : { sub: "e2e", name: "E2E", email: "e2e@st0.app", groups: [] };

/** The env.yaml above this directory; a worktree lives under the checkout that holds it. */
function findEnvFile(): string {
  if (process.env.E2E_ENV_FILE) return resolve(process.env.E2E_ENV_FILE);
  for (let dir = resolve(import.meta.dirname); ; dir = dirname(dir)) {
    const candidate = join(dir, "env.yaml");
    if (existsSync(candidate)) return candidate;
    if (dirname(dir) === dir)
      throw new Error("no env.yaml above e2e/ — set E2E_ENV_FILE");
  }
}

/** The backend's settings as env.yaml gives them, which a spec reads to act as the instance does. */
export function backendEnv(): Record<string, string | undefined> {
  return loadServiceEnv(SERVICE, findEnvFile());
}

/** The cookies a browser context needs to be `user` on `baseURL`; `iat` backdates the sign-in. */
export function sessionCookies(
  baseURL: string,
  user: SessionUser = E2E_USER,
  iat?: number,
) {
  const env = backendEnv();
  const secret = env.SESSION_SECRET;
  if (!secret) throw new Error(`env.yaml has no SESSION_SECRET for ${SERVICE}`);
  const header = sessionCookieHeader(user, {
    cookieName: COOKIE_NAME,
    secret,
    iat,
  });
  const { hostname } = new URL(baseURL);
  // Matches what the backend sets: Domain=COOKIE_DOMAIN with SameSite=None, host-only and Lax otherwise.
  const domain = env.COOKIE_DOMAIN || hostname;
  const sameSite = env.COOKIE_DOMAIN ? ("None" as const) : ("Lax" as const);
  return header.split("; ").map((pair) => {
    // At the FIRST `=`: split's limit truncates, and a base64 value ends in `=` padding.
    const at = pair.indexOf("=");
    const name = pair.slice(0, at);
    const value = pair.slice(at + 1);
    return {
      name,
      value,
      domain,
      path: "/",
      httpOnly: true,
      secure: true,
      sameSite,
    };
  });
}
