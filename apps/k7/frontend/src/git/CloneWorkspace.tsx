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
import { RemoteDialog } from "./RemoteDialog";

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
  const [name, setName] = useState("");
  const { folder, error: badName } = workspaceFolderName(name);
  return (
    <>
      <Button
        size="sm"
        variant="secondary"
        disabled={Boolean(blocked) || !t}
        title={blocked || m["git.cloneHint"]()}
        onClick={() => setOpen(true)}
      >
        <GitBranch className="h-4 w-4" /> {m["git.clone"]()}
      </Button>
      <RemoteDialog
        open={open}
        onClose={() => setOpen(false)}
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
    </>
  );
}
