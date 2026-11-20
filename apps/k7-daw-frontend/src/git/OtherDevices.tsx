// OtherDevices.tsx — "On your other devices": the workspaces the person added elsewhere that have a git remote and are
// not here, each with a Clone that runs the clone dialog with the remote and the name already filled in.
// What a reader gets wrong: a workspace with no remote is not listed, and a clone is the person's act, never automatic.

import { Button } from "@studio/ui/button";
import { Cloud, GitBranch } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
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
    <li
      data-roam={entry.id}
      className="flex items-center justify-between gap-3 py-1.5"
    >
      <span className="flex min-w-0 items-center gap-2">
        <Cloud className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="truncate">{entry.name}</span>
        <span className="truncate text-muted-foreground text-xs">
          {remoteName(remote)}
        </span>
      </span>
      <Button
        size="sm"
        variant="secondary"
        disabled={Boolean(blocked) || !t}
        title={blocked || m["git.cloneHint"]()}
        onClick={() => {
          setName(cloneName(entry.name, taken));
          setOpen(true);
        }}
      >
        <GitBranch className="h-4 w-4" /> {m["roam.clone"]()}
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
    </li>
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
    <section
      aria-label={m["roam.title"]()}
      className="rounded-md border border-border px-3 py-2 text-sm"
    >
      <h2
        title={m["roam.hint"]()}
        className="font-medium text-muted-foreground text-xs"
      >
        {m["roam.title"]()}
      </h2>
      <ul className="divide-y divide-border">
        {entries.map((e) => (
          <CloneRow
            key={e.id}
            entry={e}
            taken={local}
            blocked={blocked}
            onCloned={() => onCloned(e.id)}
          />
        ))}
      </ul>
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
