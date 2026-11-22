// useForges.ts — every forge account this device offers, signed in or not, read once from `GET /git/forges`.
// What a reader gets wrong: a failed read is "no account", so a host that cannot answer shows today's plain dialog.

import { useCallback, useEffect, useState } from "react";
import type { ForgeAccount } from "./forgeState";
import { type GitTransport, git } from "./gitApi";

export function useForges(t: GitTransport | null) {
  const [accounts, setAccounts] = useState<ForgeAccount[] | null>(null);
  const refresh = useCallback(async () => {
    if (!t) return;
    try {
      setAccounts((await git.forges(t)).forges);
    } catch {
      setAccounts([]);
    }
  }, [t]);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  return { accounts, refresh };
}
