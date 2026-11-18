// localUser.ts — the desktop's and the phone's one account, which a device's backend signs every session in as.
// The Rust front signs the same identity in (apps/k7/backend-rs/docs/routes.md); its fields must stay these.

import type { SessionUser } from "@studio/oidc";

/** The desktop's signed-out user: the dev stub's identity, so its workspaces stay where they were. */
export const LOCAL_USER: SessionUser = {
  sub: "dev-user",
  name: "Local user",
  email: "dev@example.com",
  groups: [],
  local: true,
};
