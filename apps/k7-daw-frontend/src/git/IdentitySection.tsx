// IdentitySection.tsx — who this device is: the name and email that sign its commits and head the user menu (Settings).
// It saves itself (debounced, and on blur), with a quiet note. One identity per device, shared with the commit dialog through ./identity (apps/k7-daw-backend/docs/files/git.md).

import { Input } from "@studio/ui/input";
import { useEffect, useMemo, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { Row, Section } from "../settings/Section";
import { useAccountFiles } from "../settings/useAccountFiles";
import { GitHubAccount } from "./GitHubAccount";
import { explain, type GitTransport } from "./gitApi";
import { saveIdentity, useIdentity } from "./identity";
import { accountTransport } from "./transports";

/** How long "Saved" stays beside the field. */
const SAVED_FLASH_MS = 3000;

export function IdentitySection() {
  const known = useIdentity();
  const files = useAccountFiles();
  const t = useMemo<GitTransport | null>(
    () => (files ? accountTransport(files) : null),
    [files],
  );
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  // What the last save came to, and which field it is said under; "Saved" fades by itself.
  const [said, setSaid] = useState<{
    field: "name" | "email";
    text: string;
    error: boolean;
  } | null>(null);

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
      setSaid({ field: "email", text: m["git.identitySaved"](), error: false });
    } catch (e) {
      // The route names no field: an empty name is the name's, anything else the email's.
      setSaid({
        field: name.trim() ? "email" : "name",
        text: explain(e),
        error: true,
      });
    }
  };
  // The latest save, so a timer set by an older render never writes older text.
  const latest = useRef(save);
  latest.current = save;
  // "Saved" is a flash: it goes after a moment, and an error goes with the next edit.
  useEffect(() => {
    if (!said || said.error) return;
    const id = setTimeout(() => setSaid(null), SAVED_FLASH_MS);
    return () => clearTimeout(id);
  }, [said]);
  // An edit saves itself once the typing pauses; leaving a field saves at once.
  useEffect(() => {
    if (!known) return;
    setSaid((s) => (s?.error ? null : s));
    const id = setTimeout(() => void latest.current(), 800);
    return () => clearTimeout(id);
  }, [name, email, known]);
  if (!t) return null;
  const ready = known !== null;
  return (
    <>
      <Section title={m["git.identityTitle"]()} hint={m["git.identityHint"]()}>
        <Row
          label={m["git.identityName"]()}
          detail={said?.field === "name" && !said.error ? said.text : undefined}
          error={said?.field === "name" && said.error ? said.text : undefined}
        >
          <Input
            aria-label={m["git.identityName"]()}
            value={name}
            disabled={!ready}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => void save()}
          />
        </Row>
        <Row
          label={m["git.identityEmail"]()}
          detail={
            said?.field === "email" && !said.error ? said.text : undefined
          }
          error={said?.field === "email" && said.error ? said.text : undefined}
        >
          <Input
            aria-label={m["git.identityEmail"]()}
            value={email}
            disabled={!ready}
            onChange={(e) => setEmail(e.target.value)}
            onBlur={() => void save()}
          />
        </Row>
      </Section>
      <GitHubAccount t={t} />
    </>
  );
}
