// ForgejoSection.tsx — Settings → Services, the Forgejo servers: one section each (who is signed in, Sign in, Remove), and Add.
// What a reader gets wrong: signing in again is the same flow as the first sign-in; the server's grant is per application.

import { Button } from "@studio/ui/button";
import { useMemo, useState } from "react";
import { m } from "../paraglide/messages.js";
import { Row, Section } from "../settings/Section";
import { useAccountFiles } from "../settings/useAccountFiles";
import { AddForgejoServer } from "./AddForgejoServer";
import { ForgejoSignIn } from "./ForgejoSignIn";
import {
  accountLabel,
  droppedSignIns,
  type ForgeAccount,
  forgejoServers,
} from "./forgeState";
import { explain, type GitTransport, git } from "./gitApi";
import { accountTransport } from "./transports";
import { useForges } from "./useForges";

export function ForgejoSection() {
  const files = useAccountFiles();
  const t = useMemo<GitTransport | null>(
    () => (files ? accountTransport(files) : null),
    [files],
  );
  const { accounts, refresh } = useForges(t);
  if (!t || !accounts) return null;
  return <ForgejoRows t={t} accounts={accounts} refresh={refresh} />;
}

/** The sections once the accounts are known; apart so a test renders each state without a backend. */
export function ForgejoRows({
  t,
  accounts,
  refresh,
}: {
  t: GitTransport;
  accounts: ForgeAccount[];
  refresh: () => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  return (
    <>
      {forgejoServers(accounts).map((a) => (
        <ServerRows key={a.id} t={t} account={a} refresh={refresh} />
      ))}
      <Section
        title={m["git.fj.title"]()}
        hint={adding ? undefined : m["git.fj.intro"]()}
      >
        {adding ? (
          <AddForgejoServer
            t={t}
            onAdded={() => {
              setAdding(false);
              void refresh();
            }}
            onCancel={() => setAdding(false)}
          />
        ) : (
          <Row label={m["git.fj.addLabel"]()}>
            <Button size="sm" onClick={() => setAdding(true)}>
              {m["git.fj.add"]()}
            </Button>
          </Row>
        )}
      </Section>
    </>
  );
}

function ServerRows({
  t,
  account,
  refresh,
}: {
  t: GitTransport;
  account: ForgeAccount;
  refresh: () => Promise<void>;
}) {
  const [signing, setSigning] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const id = account.id;
  const name = accountLabel(account);
  const inside = Boolean(account.login);
  const dropped = !inside && droppedSignIns.has(id);
  const remove = async () => {
    try {
      await git.forgeRemove(t, id);
      await refresh();
    } catch (e) {
      setError(explain(e));
      setRemoving(false);
    }
  };
  if (signing)
    return (
      <Section title={name}>
        <div className="px-3.5 py-2.5">
          <ForgejoSignIn
            t={t}
            server={{ id, name }}
            onDone={(_login, keyNote) => {
              droppedSignIns.delete(id);
              setNote(keyNote ?? "");
              setSigning(false);
              void refresh();
            }}
            onCancel={() => setSigning(false)}
          />
        </div>
      </Section>
    );
  if (removing)
    return (
      <Section title={name}>
        <Row
          label={m["git.fj.removeAsk"]({ server: name })}
          detail={
            account.synced
              ? m["git.fj.removeNoteSynced"]()
              : m["git.fj.removeNote"]()
          }
        >
          <Button size="sm" variant="destructive" onClick={() => void remove()}>
            {m["git.fj.removeConfirm"]()}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setRemoving(false)}>
            {m["common.cancel"]()}
          </Button>
        </Row>
      </Section>
    );
  return (
    <Section title={name}>
      <Row
        label={
          inside
            ? m["git.gh.signedAs"]({ login: account.login ?? "" })
            : dropped
              ? m["git.fj.signedOut"]()
              : m["git.gh.notSignedIn"]()
        }
        detail={note || account.url}
        error={error || undefined}
      >
        {!inside && (
          <Button size="sm" onClick={() => setSigning(true)}>
            {dropped ? m["git.fj.signInAgain"]() : m["git.fj.signIn"]()}
          </Button>
        )}
        <Button size="sm" variant="secondary" onClick={() => setRemoving(true)}>
          {m["git.fj.remove"]()}
        </Button>
      </Row>
    </Section>
  );
}
