// WorkspaceMenu.tsx — the ⋮ at the end of a workspace's row: connect it to git, or delete it (asks for the NAME).
// See ../../docs/projects.md.

import { askText } from "@studio/project-shell/ask";
import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { GitBranch, Loader2, Ellipsis, Trash2 } from "@studio/ui/icons";
import { EXAMPLES_WORKSPACE } from "@studio/web-protocol/projects";
import { useState } from "react";
import { ConnectWorkspace } from "../git/ConnectWorkspace";
import { useGitTransport } from "../git/GitContext";
import { gitMenuEntries, menuView } from "../git/gitState";
import { useGitState } from "../git/useGitState";
import { m } from "../paraglide/messages.js";

export function WorkspaceMenu({
  workspace,
  blocked,
  onRemove,
  projectCount = 0,
  onConnected,
}: {
  workspace: string;
  /** How many projects the delete takes with it — said in the confirmation. */
  projectCount?: number;
  /** Why nothing here can run — no file service, or one busy. Empty when it can. */
  blocked: string;
  onRemove: () => Promise<void>;
  /** After the workspace was connected, so the page reads it again. */
  onConnected?: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [connecting, setConnecting] = useState(false);
  // One read when the menu opens: the row's own badge keeps the state fresh the rest of the time.
  const git = useGitState(useGitTransport(), workspace, "manual");
  const view = menuView(git.state);

  // The one act here that cannot be undone, so it asks for the NAME rather than a click a habit could give.
  const remove = async () => {
    const typed = await askText({
      title: m["ws.removeTitle"]({ workspace }),
      body: m["ws.removeBody"]({ count: String(projectCount) }),
      placeholder: workspace,
      confirmLabel: m["ws.delete"](),
      validate: (v) => (v.trim() === workspace ? "" : m["ws.typeName"]()),
    });
    if (typed?.trim() !== workspace) return;
    setBusy(true);
    try {
      await onRemove();
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <DropdownMenu onOpenChange={(open) => open && void git.refresh()}>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            className="shrink-0 text-fg-2"
            title={blocked || m["ws.menu"]({ workspace })}
            aria-label={m["ws.menu"]({ workspace })}
            disabled={busy || Boolean(blocked)}
          >
            {busy ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Ellipsis className="size-5" />
            )}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-56">
          {gitMenuEntries(view, workspace === EXAMPLES_WORKSPACE).map(
            (entry) =>
              entry === "connected" && view.connected ? (
                <DropdownMenuItem
                  key={entry}
                  disabled
                  className="gap-2 text-xs"
                  title={view.remote}
                >
                  <GitBranch className="h-3.5 w-3.5" />
                  <span className="truncate">
                    {m["git.menuConnected"]({ remote: view.remote })}
                  </span>
                </DropdownMenuItem>
              ) : (
                <DropdownMenuItem
                  key={entry}
                  className="gap-2 text-xs"
                  onSelect={() => setConnecting(true)}
                >
                  <GitBranch className="h-3.5 w-3.5" />
                  {entry === "change"
                    ? m["git.menuChange"]()
                    : m["git.menuConnect"]()}
                </DropdownMenuItem>
              ),
          )}
          {/* In red: the one act on this row that cannot be undone at all. */}
          <DropdownMenuItem
            className="gap-2 text-destructive text-xs"
            onSelect={() => void remove()}
          >
            <Trash2 className="h-3.5 w-3.5" />
            {m["ws.deleteEllipsis"]()}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ConnectWorkspace
        workspace={workspace}
        open={connecting}
        changing={view.connected}
        onClose={() => setConnecting(false)}
        onConnected={() => onConnected?.()}
      />
    </>
  );
}
