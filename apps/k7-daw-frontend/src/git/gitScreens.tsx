// gitScreens.tsx — the one entry to the Conflict screen: the header, the bar and the workspace card call its opener and
// mount one host. Save runs in place (saveAct.ts); Sync settings and Clone are home pages (gitPages.ts).
import { useSyncExternalStore } from "react";
import { GitProvider } from "./GitContext";
import type { GitTransport } from "./gitApi";
import { announceGitMoved } from "./gitBus";
import { MergeDialog } from "./MergeDialog";

let current: string | null = null;
const listeners = new Set<() => void>();

function show(next: string | null): void {
  current = next;
  for (const fn of listeners) fn();
}

/** Open the Conflict screen of a merge waiting in `workspace`. */
export const openConflict = (workspace: string): void => show(workspace);

const subscribe = (fn: () => void) => {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
};

/** Mounted once per page, where a transport is known: draws the Conflict screen an opener asked for. */
export function GitScreensHost({
  transport,
}: {
  transport: GitTransport | null;
}) {
  const workspace = useSyncExternalStore(subscribe, () => current);
  if (!workspace) return null;
  return (
    <GitProvider value={transport}>
      <MergeDialog
        transport={transport}
        workspace={workspace}
        open
        onClose={() => show(null)}
        onDone={() => announceGitMoved(workspace)}
      />
    </GitProvider>
  );
}
