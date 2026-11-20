// GitBar.tsx — a workspace under git: its remote, "N to pull", "N to push", Commit, Push, Pull, and the way into a
// merge. `BarBody` draws a state; `GitBar` feeds it. What a reader gets wrong: nothing here decides a rule (gitState.ts).

import { Button } from "@studio/ui/button";
import {
  ArrowDown,
  ArrowUp,
  GitBranch,
  GitCommitHorizontal,
  GitMerge,
  Loader2,
} from "@studio/ui/icons";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { CommitDialog } from "./CommitDialog";
import { useGitTransport } from "./GitContext";
import type { GitTransport } from "./gitApi";
import { type BarView, barView, remoteName } from "./gitState";
import type { GitState, PullState } from "./gitTypes";
import { MergeDialog } from "./MergeDialog";
import { type GitAct, type GitHandle, useGitState } from "./useGitState";

/** The sentence of a pull's outcome, none for the ones that need no word. */
const pulledSentence = (p: PullState | null): string | null =>
  p === "up-to-date"
    ? m["git.pulledNothing"]()
    : p === "empty-remote"
      ? m["git.emptyRemote"]()
      : p === "merged"
        ? m["git.pulledMerged"]()
        : null;

export function BarBody({
  state,
  busy = null,
  pulled = null,
  failure = null,
  onAct,
  onCommit,
  onMerge,
  row = false,
}: {
  /** The home row's badge: no band, the branch in place of the remote's headline. */
  row?: boolean;
  state: GitState | null;
  busy?: GitAct | null;
  pulled?: PullState | null;
  failure?: string | null;
  onAct: (a: GitAct) => void;
  onCommit: () => void;
  onMerge: () => void;
}) {
  const v: BarView = barView(state);
  if (!state || !v.show) return null;
  const spin = (a: GitAct) =>
    busy === a ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null;
  const said = pulledSentence(pulled);
  return (
    <section
      aria-label={m["git.bar"]()}
      className={
        row
          ? "flex min-w-0 flex-col gap-1 font-normal text-xs"
          : "flex shrink-0 flex-col gap-1 border-b bg-muted/40 px-3 py-1 text-xs"
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        <GitBranch className="h-3.5 w-3.5 text-muted-foreground" />
        <span
          className={row ? "shrink-0" : "min-w-0 flex-1 truncate"}
          title={v.remote ? remoteName(v.remote) : undefined}
        >
          {row
            ? v.branch
            : v.remote
              ? m["git.headline"]({ remote: v.remote, branch: v.branch })
              : m["git.headlineLocal"]({ branch: v.branch })}
        </span>
        {v.toPull > 0 && (
          <span className="tabular-nums text-muted-foreground">
            {(v.toPull === 1 ? m["git.toPull.one"] : m["git.toPull.other"])({
              n: v.toPull,
            })}
          </span>
        )}
        {v.toPush > 0 && (
          <span className="tabular-nums text-muted-foreground">
            {(v.toPush === 1 ? m["git.toPush.one"] : m["git.toPush.other"])({
              n: v.toPush,
            })}
          </span>
        )}
        {v.changed > 0 && v.merge === null && (
          <span className="tabular-nums text-muted-foreground">
            {(v.changed === 1 ? m["git.changed.one"] : m["git.changed.other"])({
              n: v.changed,
            })}
          </span>
        )}
        {v.merge !== null && (
          <Button size="xs" onClick={onMerge}>
            <GitMerge className="h-3.5 w-3.5" />
            {(v.merge === 1
              ? m["git.mergeOpen.one"]
              : m["git.mergeOpen.other"])({
              n: v.merge,
            })}
          </Button>
        )}
        {v.commit && (
          <Button size="xs" disabled={!!busy} onClick={onCommit}>
            <GitCommitHorizontal className="h-3.5 w-3.5" />
            {m["git.commit"]()}
          </Button>
        )}
        {v.remote && v.merge === null && (
          <>
            <Button
              size="xs"
              variant="secondary"
              disabled={!!busy || !v.pull}
              title={
                v.pullBlocked === "commitFirst"
                  ? m["git.commitFirst"]()
                  : undefined
              }
              onClick={() => onAct("pull")}
            >
              {spin("pull") ?? <ArrowDown className="h-3.5 w-3.5" />}
              {v.pullBlocked === "commitFirst"
                ? m["git.commitFirstShort"]()
                : m["git.pull"]()}
            </Button>
            <Button
              size="xs"
              variant="secondary"
              disabled={!!busy || !v.push}
              title={
                v.pushBlocked === "pullFirst" ? m["git.pullFirst"]() : undefined
              }
              onClick={() => onAct("push")}
            >
              {spin("push") ?? <ArrowUp className="h-3.5 w-3.5" />}
              {v.pushBlocked === "pullFirst"
                ? m["git.pullFirstShort"]()
                : m["git.push"]()}
            </Button>
          </>
        )}
      </div>
      {said && <p className="text-muted-foreground">{said}</p>}
      {v.fetchError && !failure && (
        <p className="text-destructive">
          {m["git.fetchFailed"]({ error: v.fetchError })}
        </p>
      )}
      {failure && <p className="text-destructive">{failure}</p>}
    </section>
  );
}

/** The bar over a handle its host already reads, so the host can lock its editors from the same state. */
export function GitBarOf({
  git: g,
  transport,
  workspace,
  row = false,
}: {
  git: GitHandle;
  transport: GitTransport | null;
  workspace: string;
  row?: boolean;
}) {
  const [committing, setCommitting] = useState(false);
  const [merging, setMerging] = useState(false);
  // A pull that diverged opens the merge at once: the person was waiting for it.
  useEffect(() => {
    if (g.pulled === "conflicts") setMerging(true);
  }, [g.pulled]);
  return (
    <>
      <BarBody
        row={row}
        state={g.state}
        busy={g.busy}
        pulled={g.pulled}
        failure={g.failure}
        onAct={(a) => void g.act(a)}
        onCommit={() => setCommitting(true)}
        onMerge={() => setMerging(true)}
      />
      <CommitDialog
        transport={transport}
        workspace={workspace}
        open={committing}
        onClose={() => setCommitting(false)}
        onDone={() => void g.refresh()}
      />
      <MergeDialog
        transport={transport}
        workspace={workspace}
        open={merging}
        onClose={() => setMerging(false)}
        onDone={() => void g.refresh()}
      />
    </>
  );
}

/** The bar of one workspace, reading its own state; draws nothing while it is not under git. */
export function WorkspaceGitBar({
  transport,
  workspace,
}: {
  transport: GitTransport | null;
  workspace: string;
}) {
  const g = useGitState(transport, workspace);
  return <GitBarOf git={g} transport={transport} workspace={workspace} />;
}

/** The badge on a workspace's home row: its branch, counts and Commit, Push and Pull; nothing while it is not under git. */
export function WorkspaceGitRow({ workspace }: { workspace: string }) {
  const transport = useGitTransport();
  const g = useGitState(transport, workspace, "once");
  return <GitBarOf git={g} transport={transport} workspace={workspace} row />;
}
