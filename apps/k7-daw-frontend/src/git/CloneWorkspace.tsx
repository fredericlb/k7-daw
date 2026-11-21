// CloneWorkspace.tsx — "Clone from git": brings a repository onto this device as a new workspace of its own name.
// The name is the folder, so it is checked as any workspace's is before the call.

import { Button } from "@studio/ui/button";
import { GitBranch } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { workspaceFolderName } from "../projects/projectName";
import { useGitTransport } from "./GitContext";
import { git } from "./gitApi";
import { shortName } from "./githubState";
import { RemoteDialog } from "./RemoteDialog";

/** The clone dialog alone, for a menu item that opens it (the Home "New" menu). */
export function CloneDialog({
  open,
  onClose,
  onCloned,
}: {
  open: boolean;
  onClose: () => void;
  onCloned: () => void;
}) {
  const t = useGitTransport();
  const [name, setName] = useState("");
  const { folder, error: badName } = workspaceFolderName(name);
  return (
    <RemoteDialog
      open={open}
      onClose={onClose}
      title={m["git.cloneTitle"]()}
      description={m["git.cloneBody"]()}
      submit={m["git.cloneSubmit"]()}
      step={m["git.cloning"]()}
      canSubmit={Boolean(folder)}
      onRepoPicked={(r) => setName(shortName(r.fullName))}
      before={
        <>
          <Input
            autoFocus
            aria-label={m["git.cloneName"]()}
            placeholder={m["git.cloneName"]()}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          {badName && name.trim() && (
            <p className="text-error type-secondary">{badName}</p>
          )}
        </>
      }
      onSubmit={async (r) => {
        if (!t || !folder) return;
        await git.clone(t, folder, r);
        setName("");
        onCloned();
      }}
    />
  );
}

export function CloneWorkspace({
  blocked,
  onCloned,
}: {
  /** Why nothing can run — no file service — or "". */
  blocked: string;
  onCloned: () => void;
}) {
  const t = useGitTransport();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        size="sm"
        variant="secondary"
        disabled={Boolean(blocked) || !t}
        title={blocked || m["git.cloneHint"]()}
        onClick={() => setOpen(true)}
      >
        <GitBranch className="size-4" /> {m["git.clone"]()}
      </Button>
      <CloneDialog
        open={open}
        onClose={() => setOpen(false)}
        onCloned={onCloned}
      />
    </>
  );
}
