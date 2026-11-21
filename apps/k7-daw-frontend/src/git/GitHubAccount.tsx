// GitHubAccount.tsx — Settings → Devices: who this device is signed in as on GitHub, and Sign out.
// What a reader gets wrong: signing out removes the token only; the key k7 added to GitHub stays, and the page says so.

import { Button } from "@studio/ui/button";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { Row, Section } from "../settings/Section";
import { explain, type GitTransport, git } from "./gitApi";
import { signedIn } from "./githubState";
import { useGithub } from "./useGithub";

const KEYS_URL = "https://github.com/settings/keys";

export function GitHubAccount({ t }: { t: GitTransport }) {
  const { status, refresh } = useGithub(t);
  const [error, setError] = useState("");
  if (!signedIn(status)) return null;
  const out = async () => {
    try {
      await git.githubSignout(t);
      await refresh();
    } catch (e) {
      setError(explain(e));
    }
  };
  return (
    <Section title={m["git.gh.account"]()} hint={m["git.gh.signOutNote"]()}>
      <Row
        label={m["git.gh.accountLogin"]({ login: status?.login ?? "" })}
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
        <Button size="sm" variant="secondary" onClick={() => void out()}>
          {m["git.gh.signOut"]()}
        </Button>
      </Row>
    </Section>
  );
}
