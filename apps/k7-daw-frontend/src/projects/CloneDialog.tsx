// CloneDialog.tsx — "Clone from git" as a dialog the New menu opens: the repository's address and the workspace's name.
// What a reader gets wrong: it is the dialog of git/CloneWorkspace.tsx without that file's own button, which a menu item replaces.

import { Input } from "@studio/ui/input";
import { useState } from "react";
import { useGitTransport } from "../git/GitContext";
import { git } from "../git/gitApi";
import { RemoteDialog } from "../git/RemoteDialog";
import { m } from "../paraglide/messages.js";
import { workspaceFolderName } from "./projectName";

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
        setName("");
        onCloned();
      }}
    />
  );
}
