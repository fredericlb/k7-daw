// WorkspaceMenu.tsx — the ⋮ at the end of a workspace's row: its Sync settings, or delete it (asks for the NAME).
// See ../../docs/projects.md.

import { askText } from "@studio/project-shell/ask";
import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  Ellipsis,
  FolderOpen,
  GitBranch,
  Loader2,
  Trash2,
} from "@studio/ui/icons";
import { EXAMPLES_WORKSPACE } from "@studio/web-protocol/projects";
import { useEffect, useState } from "react";
import { useGitTransport } from "../git/GitContext";
import { onGitMoved } from "../git/gitBus";
import { openSyncSettings } from "../git/gitPages";
import { gitMenuEntries, menuView } from "../git/gitState";
import { useGitState } from "../git/useGitState";
import { m } from "../paraglide/messages.js";

export function WorkspaceMenu({
  workspace,
  blocked,
  onRemove,
  projectCount = 0,
  onConnected,
  onBrowse,
  locked = "",
}: {
  workspace: string;
  /** How many projects the delete takes with it — said in the confirmation. */
  projectCount?: number;
  /** Why nothing here can run — no file service, or one busy. Empty when it can. */
  blocked: string;
  onRemove: () => Promise<void>;
  /** After the workspace was connected, so the page reads it again. */
  onConnected?: () => void;
  /** Opens the workspace's file tree, the read-only developer view. */
  onBrowse: () => void;
  /** The sentence of a merge waiting here, which stops the delete; empty when none. */
  locked?: string;
}) {
  const [busy, setBusy] = useState(false);
  // One read when the menu opens: the row's own badge keeps the state fresh the rest of the time.
  const git = useGitState(useGitTransport(), workspace, "manual");
  const view = menuView(git.state);

  // Sync settings is a page of its own: its announcement is how this page reads the workspace again.
  useEffect(
    () => onGitMoved(workspace, () => onConnected?.()),
    [workspace, onConnected],
  );

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
    <DropdownMenu onOpenChange={(open) => open && void git.refresh()}>
      <DropdownMenuTrigger asChild disabled={busy || Boolean(blocked)}>
        <Button
          variant="ghost"
          size="icon"
          className="shrink-0 text-fg-2"
          title={blocked || m["ws.menu"]({ workspace })}
          aria-label={m["ws.menu"]({ workspace })}
          disabled={busy || Boolean(blocked)}
          disabledReason={blocked || undefined}
        >
          {busy ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Ellipsis className="size-5" />
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        {gitMenuEntries(view, workspace === EXAMPLES_WORKSPACE).map((entry) =>
          entry === "connected" && view.connected ? (
            <DropdownMenuItem key={entry} disabled title={view.remote}>
              <GitBranch />
              <span className="truncate">
                {m["git.menuConnected"]({ remote: view.remote })}
              </span>
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem
              key={entry}
              onSelect={() => openSyncSettings(workspace)}
            >
              <GitBranch />
              {m["git.menuSync"]()}
            </DropdownMenuItem>
          ),
        )}
        <DropdownMenuItem onSelect={onBrowse}>
          <FolderOpen />
          {m["ws.browse"]()}
        </DropdownMenuItem>
        {/* In red: the one act on this row that cannot be undone at all. */}
        <DropdownMenuItem
          className="text-destructive [&_svg]:text-destructive"
          disabled={Boolean(locked)}
          disabledReason={locked || undefined}
          title={locked || undefined}
          onSelect={() => void remove()}
        >
          <Trash2 />
          {m["ws.deleteEllipsis"]()}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
