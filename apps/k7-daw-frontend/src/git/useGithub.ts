// useGithub.ts — what the screens know of the GitHub sign-in: whether this build offers it and who is signed in.
// What a reader gets wrong: a failed read is "not offered", so a host that cannot answer shows today's dialog.

import { useCallback, useEffect, useState } from "react";
import { type GitTransport, git } from "./gitApi";
import type { GithubStatus } from "./githubState";

const NONE: GithubStatus = { available: false, scopes: [] };

export function useGithub(t: GitTransport | null) {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const refresh = useCallback(async () => {
    if (!t) return;
    try {
      setStatus(await git.githubStatus(t));
    } catch {
      setStatus(NONE);
    }
  }, [t]);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  return { status, refresh };
}
