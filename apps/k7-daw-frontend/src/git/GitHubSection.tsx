// GitHubSection.tsx — Settings → Services: Sign in with GitHub (one code, every permission), who is signed in, Sign out.
// What a reader gets wrong: this is the only place k7 signs in to GitHub; Connect and Clone only read the result.

import { Button } from "@studio/ui/button";
import { useMemo, useState } from "react";
import { m } from "../paraglide/messages.js";
import { Row, Section } from "../settings/Section";
import { useAccountFiles } from "../settings/useAccountFiles";
import { DeviceFlow } from "./DeviceFlow";
import { explain, type GitTransport, git } from "./gitApi";
import {
  GITHUB_SCOPES,
  type GithubStatus,
  hasRepoScope,
  signedIn,
} from "./githubState";
import { accountTransport } from "./transports";
import { useGithub } from "./useGithub";

const KEYS_URL = "https://github.com/settings/keys";

export function GitHubSection() {
  const files = useAccountFiles();
  const t = useMemo<GitTransport | null>(
    () => (files ? accountTransport(files) : null),
    [files],
  );
  const { status, refresh } = useGithub(t);
  if (!t || !status) return null;
  return <GitHubRows t={t} status={status} refresh={refresh} />;
}

/** The section once the status is known; apart so a test renders each state without a backend. */
export function GitHubRows({
  t,
  status,
  refresh,
}: {
  t: GitTransport;
  status: GithubStatus;
  refresh: () => Promise<void>;
}) {
  const [signing, setSigning] = useState(false);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const title = m["git.gh.account"]();
  if (!status.available)
    return (
      <Section title={title}>
        <Row label={m["git.gh.unavailable"]()} />
      </Section>
    );
  const inside = signedIn(status);
  const repos = hasRepoScope(status);
  if (signing)
    return (
      <Section title={title}>
        <div className="px-3.5 py-2.5">
          <DeviceFlow
            t={t}
            scopes={GITHUB_SCOPES}
            why={inside ? m["git.gh.repoWhy"]() : undefined}
            onDone={(_login, keyNote) => {
              setNote(keyNote ?? "");
              setSigning(false);
              void refresh();
            }}
            onCancel={() => setSigning(false)}
          />
        </div>
      </Section>
    );
  if (!inside)
    return (
      <Section title={title} hint={m["git.gh.intro"]()}>
        <Row label={m["git.gh.notSignedIn"]()}>
          <Button size="sm" onClick={() => setSigning(true)}>
            {m["git.gh.signIn"]()}
          </Button>
        </Row>
      </Section>
    );
  const out = async () => {
    try {
      await git.githubSignout(t);
      setNote("");
      await refresh();
    } catch (e) {
      setError(explain(e));
    }
  };
  return (
    <Section
      title={title}
      hint={
        status.synced
          ? m["git.gh.signOutNoteSynced"]()
          : m["git.gh.signOutNote"]()
      }
    >
      <Row
        label={m["git.gh.accountLogin"]({ login: status.login ?? "" })}
        detail={note || (repos ? undefined : m["git.gh.repoWhy"]())}
        error={error || undefined}
      >
        <a
          href={KEYS_URL}
          target="_blank"
          rel="noreferrer"
          className="type-status self-center text-fg-3 underline"
        >
          {m["git.gh.manageKeys"]()}
        </a>
        {!repos && (
          <Button size="sm" onClick={() => setSigning(true)}>
            {m["git.gh.allowRepos"]()}
          </Button>
        )}
        <Button size="sm" variant="secondary" onClick={() => void out()}>
          {m["git.gh.signOut"]()}
        </Button>
      </Row>
    </Section>
  );
}
