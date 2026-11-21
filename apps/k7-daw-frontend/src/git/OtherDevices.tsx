// OtherDevices.tsx — "On your other devices": the workspaces the person added elsewhere that have a git remote and are
// not here, each with a Clone that runs the clone dialog with the remote and the name already filled in.
// What a reader gets wrong: a workspace with no remote is not listed, and a clone is the person's act, never automatic.

import { Button } from "@studio/ui/button";
import { Cloud, GitBranch } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { RowGroup } from "@studio/ui/row-group";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { workspaceFolderName } from "../projects/projectName";
import { useGitTransport } from "./GitContext";
import { git } from "./gitApi";
import { remoteName } from "./gitState";
import { RemoteDialog } from "./RemoteDialog";
import { cloneName, type RoamEntry } from "./roamState";
import { useRoamList } from "./useRoamList";

function CloneRow({
  entry,
  taken,
  blocked,
  onCloned,
}: {
  entry: RoamEntry;
  taken: readonly string[];
  blocked: string;
  onCloned: () => void;
}) {
  const t = useGitTransport();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(() => cloneName(entry.name, taken));
  const { folder, error: badName } = workspaceFolderName(name);
  const remote = entry.remote ?? "";
  return (
    <div
      data-roam={entry.id}
      className="flex min-h-14 items-center justify-between gap-3 px-3.5 py-2"
    >
      <span className="flex min-w-0 items-center gap-3">
        <Cloud className="size-5 shrink-0 text-primary-text" />
        <span className="min-w-0">
          <span className="type-body block truncate">{entry.name}</span>
          <span className="type-status block truncate text-fg-3">
            {remoteName(remote)}
          </span>
        </span>
      </span>
      <Button
        variant="glass"
        aria-label={m["roam.cloneNamed"]({ name: entry.name })}
        disabled={Boolean(blocked) || !t}
        disabledReason={blocked || undefined}
        title={blocked || m["git.cloneHint"]()}
        onClick={() => {
          setName(cloneName(entry.name, taken));
          setOpen(true);
        }}
      >
        <GitBranch className="size-5" /> {m["roam.clone"]()}
      </Button>
      <RemoteDialog
        open={open}
        onClose={() => setOpen(false)}
        title={m["roam.cloneTitle"]({ name: entry.name })}
        description={m["roam.cloneBody"]()}
        submit={m["git.cloneSubmit"]()}
        step={m["git.cloning"]()}
        canSubmit={Boolean(folder)}
        initialUrl={remote}
        before={
          <>
            <Input
              aria-label={m["git.cloneName"]()}
              placeholder={m["git.cloneName"]()}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            {badName && name.trim() && (
              <p className="text-destructive text-xs">{badName}</p>
            )}
          </>
        }
        onSubmit={async (r) => {
          if (!t || !folder) return;
          await git.clone(t, folder, r);
          onCloned();
        }}
      />
    </div>
  );
}

/** The section over the entries to offer; nothing when there are none. */
export function OtherList({
  entries,
  local,
  blocked,
  onCloned,
}: {
  entries: readonly RoamEntry[];
  /** The names of this device's workspaces, so a clone's name does not start on a taken one. */
  local: readonly string[];
  blocked: string;
  onCloned: (id: string) => void;
}) {
  if (entries.length === 0) return null;
  return (
    <section aria-label={m["roam.title"]()} className="flex flex-col gap-2">
      <h2 className="type-caps px-1 text-fg-3">{m["roam.title"]()}</h2>
      <RowGroup>
        {entries.map((e) => (
          <CloneRow
            key={e.id}
            entry={e}
            taken={local}
            blocked={blocked}
            onCloned={() => onCloned(e.id)}
          />
        ))}
      </RowGroup>
      <p className="type-status px-1 text-fg-3">{m["roam.hint"]()}</p>
    </section>
  );
}

export function OtherDevices({
  local,
  blocked,
  onChanged,
}: {
  local: readonly string[];
  blocked: string;
  onChanged: () => void;
}) {
  const { entries, cloned, reload } = useRoamList(useGitTransport());
  return (
    <OtherList
      entries={entries}
      local={local}
      blocked={blocked}
      onCloned={(id) => {
        cloned(id);
        onChanged();
        reload();
      }}
    />
  );
}
