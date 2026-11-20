// gitScreens.tsx — the one entry to the git screens: the header and the workspace card call these three
// openers and mount one host, and never reach a dialog. The bodies belong to the git screens' own step.
import { useSyncExternalStore } from "react";
import { CommitDialog } from "./CommitDialog";
import { ConnectWorkspace } from "./ConnectWorkspace";
import { GitProvider } from "./GitContext";
import type { GitTransport } from "./gitApi";
import { announceGitMoved } from "./gitBus";
import { MergeDialog } from "./MergeDialog";

type Screen = {
  kind: "commit" | "conflict" | "connect";
  workspace: string;
  /** Connect only: the workspace already has a remote, which this one replaces. */
  changing?: boolean;
} | null;

let current: Screen = null;
const listeners = new Set<() => void>();

function show(next: Screen): void {
  current = next;
  for (const fn of listeners) fn();
}

/** Open the Commit & Push dialog over `workspace`. */
export const openCommitAndPush = (workspace: string): void =>
  show({ kind: "commit", workspace });

/** Open the Conflict screen of a merge waiting in `workspace`. */
export const openConflict = (workspace: string): void =>
  show({ kind: "conflict", workspace });

/** Open the Connect-to-git dialog of `workspace`; `changing` when it replaces a remote. */
export const openConnect = (
  workspace: string,
  opts: { changing?: boolean } = {},
): void => show({ kind: "connect", workspace, changing: opts.changing });

const subscribe = (fn: () => void) => {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
};

/** Mounted once per page, where a transport is known: draws whichever screen an opener asked for. */
export function GitScreensHost({
  transport,
}: {
  transport: GitTransport | null;
}) {
  const screen = useSyncExternalStore(subscribe, () => current);
  if (!screen) return null;
  const { kind, workspace, changing } = screen;
  const close = () => show(null);
  const moved = () => announceGitMoved(workspace);
  return (
    <GitProvider value={transport}>
      {kind === "commit" && (
        <CommitDialog
          transport={transport}
          workspace={workspace}
          open
          onClose={close}
          onDone={moved}
          onConflict={() => show({ kind: "conflict", workspace })}
        />
      )}
      {kind === "conflict" && (
        <MergeDialog
          transport={transport}
          workspace={workspace}
          open
          onClose={close}
          onDone={moved}
        />
      )}
      {kind === "connect" && (
        <ConnectWorkspace
          workspace={workspace}
          open
          changing={changing}
          onClose={close}
          onConnected={moved}
        />
      )}
    </GitProvider>
  );
}
