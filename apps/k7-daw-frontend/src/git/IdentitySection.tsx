// IdentitySection.tsx — who this device is: the name and email that sign its commits and head the user menu (Settings).
// It saves itself (debounced, and on blur), with a quiet note. One identity per device, shared with the commit dialog through ./identity (apps/k7-daw-backend/docs/files/git.md).

import { Input } from "@studio/ui/input";
import { useEffect, useRef, useState } from "react";
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
    // Keep a draft that only differs by spaces: the save trims it, and the field must not jump under the caret.
    setName((n) => (n.trim() === known.name ? n : known.name));
    setEmail((e) => (e.trim() === known.email ? e : known.email));
  }, [known]);

  const save = async () => {
    if (!t || !known) return;
    if (name.trim() === known.name && email.trim() === known.email) return;
    try {
      await saveIdentity(t, { name, email });
      setSaid(m["git.identitySaved"]());
    } catch (e) {
      setSaid(explain(e));
    }
  };
  // The latest save, so a timer set by an older render never writes older text.
  const latest = useRef(save);
  latest.current = save;
  // An edit saves itself once the typing pauses; leaving a field saves at once.
  useEffect(() => {
    if (!known) return;
    const id = setTimeout(() => void latest.current(), 800);
    return () => clearTimeout(id);
  }, [name, email, known]);
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
          onBlur={() => void save()}
        />
      </Row>
      <Row label={m["git.identityEmail"]()} detail={said || undefined}>
        <Input
          aria-label={m["git.identityEmail"]()}
          value={email}
          disabled={!ready}
          onChange={(e) => setEmail(e.target.value)}
          onBlur={() => void save()}
        />
      </Row>
    </Section>
  );
}
