// ConnectWorkspace.tsx — "Connect to git" on a workspace: the address and sign-in, then the first fetch proves them.
// A host without large-file storage is said after the connection, not refused: the push is what it refuses.

import { askNotice } from "@studio/project-shell/ask";
import { showToast } from "@studio/project-shell/toast";
import { m } from "../paraglide/messages.js";
import { useGitTransport } from "./GitContext";
import { git } from "./gitApi";
import { announceGitMoved } from "./gitBus";
import { remoteName } from "./gitState";
import { RemoteDialog } from "./RemoteDialog";

export function ConnectWorkspace({
  workspace,
  open,
  onClose,
  onConnected,
  changing = false,
}: {
  /** The workspace already has a remote: the dialog points it elsewhere. */
  changing?: boolean;
  workspace: string;
  open: boolean;
  onClose: () => void;
  onConnected: () => void;
}) {
  const t = useGitTransport();
  return (
    <RemoteDialog
      open={open}
      onClose={onClose}
      title={
        changing
          ? m["git.changeTitle"]({ workspace })
          : m["git.connectTitle"]({ workspace })
      }
      description={m["git.connectBody"]()}
      submit={m["git.connectSubmit"]()}
      step={m["git.connecting"]()}
      canSubmit={t !== null}
      createName={workspace}
      onSubmit={async (r) => {
        if (!t) return;
        const done = await git.connect(t, workspace, r);
        announceGitMoved(workspace);
        onConnected();
        showToast({
          title: m["git.connectedTitle"](),
          message: m["git.connectedToast"]({
            workspace,
            remote: remoteName(done.remote || r.url),
          }),
        });
        if (done.lfsWarning)
          void askNotice({
            title: m["git.noLfsTitle"](),
            body: m["git.noLfsBody"]({
              host: remoteName(done.remote || r.url),
            }),
            confirmLabel: m["common.close"](),
          });
      }}
    />
  );
}
