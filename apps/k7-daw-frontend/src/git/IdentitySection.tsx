// IdentitySection.tsx — who this device is: the name and email that sign its commits and head the user menu (Settings).
// It saves itself (debounced, and on blur), with a quiet note. One identity per device, shared with the Save act through ./identity (apps/k7-daw-backend/docs/files/git.md).

import { Input } from "@studio/ui/input";
import { useEffect, useMemo, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { Row, Section } from "../settings/Section";
import { useAccountFiles } from "../settings/useAccountFiles";
import { explain, type GitTransport } from "./gitApi";
import { saveIdentity, useIdentity } from "./identity";
import { sourceNote } from "./identitySource";
import { accountTransport } from "./transports";

/** How long "Saved" stays beside the field. */
const SAVED_FLASH_MS = 3000;

/** The source line of a field that still shows what was suggested. */
const suggested = (
  value: string,
  shown: string | undefined,
  source: string | null | undefined,
) => (value.trim() === shown ? sourceNote(source) : undefined);

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

  // A save made elsewhere lands here too.
  useEffect(() => {
    if (!known) return;
    // Keep a draft that only differs by spaces: the save trims it, and the field must not jump under the caret.
    setName((n) => (n.trim() === known.name ? n : known.name));
    setEmail((e) => (e.trim() === known.email ? e : known.email));
  }, [known]);

  const save = async () => {
    if (!t || !known) return;
    if (name.trim() === known.name && email.trim() === known.email) return;
    // A field still showing its suggestion is not typed: it is sent empty so the suggestion keeps following its source.
    const typed = (v: string, shown: string, source?: string | null) =>
      source && v.trim() === shown ? "" : v;
    try {
      await saveIdentity(t, {
        name: typed(name, known.name, known.nameSource),
        email: typed(email, known.email, known.emailSource),
      });
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
    <Section title={m["git.identityTitle"]()} hint={m["git.identityHint"]()}>
      <Row
        label={m["git.identityName"]()}
        detail={
          said?.field === "name" && !said.error
            ? said.text
            : suggested(name, known?.name, known?.nameSource)
        }
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
          said?.field === "email" && !said.error
            ? said.text
            : suggested(email, known?.email, known?.emailSource)
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
  );
}
