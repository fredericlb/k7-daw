// menuAccount.ts — the user menu's heading: the name kept in Settings (the commit author), "Local" while none is set.
// No account to ask: the identity is the device's own. See ../../docs/userMenu.md.

import { headingOf, useIdentity } from "../git/identity";

/** The heading, null until the identity is known. */
export function useMenuAccount(): string | null {
  const id = useIdentity();
  return id ? headingOf(id) : null;
}
