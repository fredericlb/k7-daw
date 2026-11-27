// ProjectSettingsTab.tsx — the settings panel's PROJECT tab: what the open song says about its
// whole mix, today its summing console. Every change is the `project …` verb, so the palette and
// this tab write the same line. See apps/k7-daw-frontend/packages/song/docs/console.md.

import { Picker } from "@studio/ui/picker";
import { SectionHeading } from "@studio/ui/section-heading";
import { sortForPeople } from "@studio/web-protocol/tree";
import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { m } from "../paraglide/messages.js";
import { PanelError } from "./PanelError";

/** The file service's answer when no project is open (`NO_ACTIVE`, k7-file-service's module_reads.rs). */
const NO_PROJECT = /^no active project/;

/** What the file service said: the systems the libraries hold, and the one the project names. */
interface Consoles {
  systems: { ref: string; label: string }[];
  current?: string;
}

export function ProjectSettingsTab({
  onRun,
}: {
  onRun: (line: string) => Promise<void>;
}) {
  // Null while asking, and left null when there is no project to ask about.
  const [consoles, setConsoles] = useState<Consoles | null>(null);
  /** Why it could not be read: no project open is an answer, anything else is a failure to show. */
  const [failed, setFailed] = useState<{
    noProject: boolean;
    text: string;
  } | null>(null);

  const load = useCallback(
    () =>
      api.consoles().then(
        (c) => {
          // The service answers in code points; the select reads for people (../../docs/listOrder.md).
          setConsoles({
            ...c,
            systems: sortForPeople(c.systems, (s) => s.label),
          });
          setFailed(null);
        },
        (e: Error) =>
          setFailed({ noProject: NO_PROJECT.test(e.message), text: e.message }),
      ),
    [],
  );
  useEffect(() => {
    void load();
  }, [load]);

  const pick = async (value: string) => {
    await onRun(`project console ${value || "none"}`);
    // Read back rather than assumed: a refused verb leaves the file as it was.
    await load();
  };

  if (failed?.noProject)
    return (
      <p className="type-status p-1 text-fg-2">{m["settings.noProject"]()}</p>
    );
  if (failed)
    return (
      <PanelError
        what={m["panel.what.project"]()}
        detail={failed.text}
        onRetry={() => void load()}
      />
    );

  const current = consoles?.current ?? "";
  // A system the file names but no library holds is kept in the list, so the select shows the truth.
  const missing = current && !consoles?.systems.some((s) => s.ref === current);

  const items = [
    { value: "", label: m["settings.consoleNone"]() },
    ...(consoles?.systems.map((c) => ({
      value: c.ref,
      label: c.label,
      detail: c.ref,
    })) ?? []),
    ...(missing ? [{ value: current, label: `${current} ?` }] : []),
  ];

  return (
    <div className="flex flex-col gap-2">
      <SectionHeading>{m["settings.console"]()}</SectionHeading>
      <Picker
        items={items}
        value={current}
        onValueChange={(v) => void pick(v)}
        label={m["settings.console"]()}
        searchLabel={m["settings.console"]()}
        emptyLabel={m["settings.consoleEmpty"]()}
      />
      <p className="type-status text-fg-3">
        {consoles && !consoles.systems.length
          ? m["settings.consoleEmpty"]()
          : m["settings.consoleHint"]()}
      </p>
    </div>
  );
}
