// authState.ts — where k7 keeps the state that makes a session or a token revocable: a dot-folder of
// the instance's data folder, so it survives a redeploy (../docs/sessions.md).

import { join } from "node:path";
import { mediatekDir } from "@studio/engine-client/paths";
import {
  createSessionRevocations,
  type SessionRevocations,
} from "@studio/oidc";

/** `$STUDIO_AUTH_DIR`, or `.auth` in the data folder. */
export function authDir(): string {
  return process.env.STUDIO_AUTH_DIR || join(mediatekDir(), ".auth");
}

/** The per-account "not before" epochs a logout everywhere writes. */
export function sessionRevocations(): SessionRevocations {
  return createSessionRevocations({
    file: join(authDir(), "session-revocations.json"),
  });
}
