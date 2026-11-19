// session.ts — a signed-in browser for the suite: the install's token as the `k7.sid` cookie.
// The token is `K7_SESSION_TOKEN` in this machine's env.yaml, which the bed starts the front with (docs/e2e.md).
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { loadServiceEnv } from "@studio/env";

const COOKIE_NAME = "k7.sid";
// The front's env.yaml section, the one that names its token.
const SERVICE = "k7-front";

/** What `/auth/me` names: the one user a device has. Specs act as it whoever they name. */
export interface SessionUser {
  sub?: string;
  name?: string;
  email?: string | null;
  groups?: string[];
  [key: string]: unknown;
}

/** The local user, the only account a device has (e2e/docs/e2e.md). */
export const E2E_USER: SessionUser = {
  sub: "dev-user",
  name: "Local user",
  email: "dev@example.com",
  groups: [],
  local: true,
};

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

/** The cookies a browser context needs to be signed in on `baseURL`. */
export function sessionCookies(baseURL: string) {
  const token = backendEnv().K7_SESSION_TOKEN;
  if (!token)
    throw new Error(`env.yaml has no K7_SESSION_TOKEN for ${SERVICE}`);
  return [
    {
      name: COOKIE_NAME,
      value: token,
      domain: new URL(baseURL).hostname,
      path: "/",
      httpOnly: true,
      secure: false,
      sameSite: "Strict" as const,
    },
  ];
}
