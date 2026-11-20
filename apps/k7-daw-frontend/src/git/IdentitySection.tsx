// IdentitySection.tsx — who this device is: the name and email that sign its commits and head the user menu (Settings).
// One identity per device, shared with the commit dialog through ./identity (apps/k7/backend-rs/docs/files/git.md).

import { Button } from "@studio/ui/button";
import { Input } from "@studio/ui/input";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { findAccountFiles } from "../settings/accountFiles";
import { Row, Section } from "../settings/Section";
import { explain, type GitTransport } from "./gitApi";
import { saveIdentity, useIdentity } from "./identity";
import { accountTransport } from "./transports";

export function IdentitySection() {
  const known = useIdentity();
  const [t, setT] = useState<GitTransport | null>(null);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [said, setSaid] = useState("");

  useEffect(() => {
    let live = true;
    void findAccountFiles(false).then((files) => {
      if (live && files) setT(accountTransport(files));
    });
    return () => {
      live = false;
    };
  }, []);

  // A save made elsewhere (the commit dialog) lands here too.
  useEffect(() => {
    if (!known) return;
    setName(known.name);
    setEmail(known.email);
  }, [known]);

  const save = async () => {
    if (!t) return;
    try {
      await saveIdentity(t, { name, email });
      setSaid(m["git.identitySaved"]());
    } catch (e) {
      setSaid(explain(e));
    }
  };
  if (!t) return null;
  const ready = known !== null;
  return (
    <Section title={m["git.identityTitle"]()} hint={m["git.identityHint"]()}>
      <Row label={m["git.identityName"]()}>
        <Input
          aria-label={m["git.identityName"]()}
          value={name}
          disabled={!ready}
          onChange={(e) => setName(e.target.value)}
        />
      </Row>
      <Row label={m["git.identityEmail"]()} detail={said || undefined}>
        <Input
          aria-label={m["git.identityEmail"]()}
          value={email}
          disabled={!ready}
          onChange={(e) => setEmail(e.target.value)}
        />
        <Button size="sm" disabled={!ready} onClick={() => void save()}>
          {m["common.save"]()}
        </Button>
      </Row>
    </Section>
  );
}
