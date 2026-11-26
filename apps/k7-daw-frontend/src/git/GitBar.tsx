// GitBar.tsx — a workspace under git, two ways: the file tree's bar (`BarBody`: remote, counts, Save, Pull, the merge)
// and the home card's pills (`CardGitPills`: Pull in place, Save, Setup sync). Save runs in place (saveAct.ts); the merge
// and Sync settings are screens of their own. What a reader gets wrong: nothing here decides a rule (gitState.ts).

import { showToast } from "@studio/project-shell/toast";
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import {
  ArrowDown,
  ArrowDownToLine,
  GitBranch,
  GitMerge,
  Loader2,
  RefreshCw,
  Save,
} from "@studio/ui/icons";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";
import { editedAgo } from "../projects/projectKind";
import { useGitTransport } from "./GitContext";
import type { GitTransport } from "./gitApi";
import { openSyncSettings } from "./gitPages";
import { openConflict } from "./gitScreens";
import { type BarView, barView, remoteName } from "./gitState";
import type { GitState, PullState } from "./gitTypes";
import { pulledSentence } from "./pulledSentence";
import { save, useSaving } from "./saveAct";
import { type GitAct, type GitHandle, useGitState } from "./useGitState";

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
        {(v.commit || v.push) && (
          <Button size="xs" disabled={!!busy} onClick={onCommit}>
            <Save className="h-3.5 w-3.5" />
            {m["git.save.label"]()}
          </Button>
        )}
        {v.remote && v.merge === null && (
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
function GitBarOf({
  git: g,
  workspace,
  row = false,
}: {
  git: GitHandle;
  workspace: string;
  row?: boolean;
}) {
  const t = useGitTransport();
  const saving = useSaving(workspace);
  // A pull that diverged opens the merge at once: the person was waiting for it.
  useEffect(() => {
    if (g.pulled === "conflicts") openConflict(workspace);
  }, [g.pulled, workspace]);
  return (
    <BarBody
      row={row}
      state={g.state}
      busy={g.busy ?? (saving ? "push" : null)}
      pulled={g.pulled}
      failure={g.failure}
      onAct={(a) => void g.act(a)}
      onCommit={() =>
        t && void save(t, workspace, { onPull: () => void g.act("pull") })
      }
      onMerge={() => openConflict(workspace)}
    />
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
  return <GitBarOf git={g} workspace={workspace} />;
}

/** How long "Updated and merged." stays on the card before the counts are all it says. */
const PULLED_NOTE_MS = 6000;

/** A workspace's git state for its home card: read once and when announced, its failures said as toasts. */
export function useCardGit(workspace: string): GitHandle {
  const g = useGitState(useGitTransport(), workspace, "once");
  useEffect(() => {
    if (g.pulled === "conflicts") openConflict(workspace);
  }, [g.pulled, workspace]);
  useEffect(() => {
    if (g.failure) showToast({ message: g.failure, variant: "error" });
  }, [g.failure]);
  return g;
}

/** What the last pull came to, said beside the pills for a few seconds. */
export function CardGitNote({ git: g }: { git: GitHandle }) {
  const said = pulledSentence(g.pulled);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!said) return setShown(false);
    setShown(true);
    const id = window.setTimeout(() => setShown(false), PULLED_NOTE_MS);
    return () => window.clearTimeout(id);
  }, [said]);
  // "Up to date" is the pills' lasting line; here only what a pull said that lasts no longer.
  if (!said || !shown || g.busy || g.pulled === "up-to-date") return null;
  return (
    <span role="status" className="type-secondary shrink-0 text-fg-2">
      {said}
    </span>
  );
}

/** The bar under a card's header while a pull runs: no percentage, the pull reports none. */
export function CardPullBar({
  git: g,
  workspace,
}: {
  git: GitHandle;
  workspace: string;
}) {
  if (g.busy !== "pull") return null;
  const v = barView(g.state);
  return (
    <div className="flex flex-col gap-1.5 px-4 pb-3">
      <div
        role="progressbar"
        aria-label={m["git.card.pullingName"]({ workspace })}
        className="h-1.5 w-full overflow-hidden rounded-[3px] bg-(--k7-progress-track)"
      >
        <div className="h-full w-1/3 animate-pulse rounded-[3px] bg-(--k7-progress-fill)" />
      </div>
      <span className="type-status text-fg-3">
        {m["git.card.pullingFrom"]({
          branch: v.branch,
          remote: v.remote ? remoteName(v.remote) : "",
        })}
      </span>
    </div>
  );
}

/** The card's git pills: Update in place, Save as one act, the merge when one waits, Setup sync when there is no remote. */
export function CardGitPills({
  git: g,
  workspace,
  demo = false,
}: {
  git: GitHandle;
  workspace: string;
  /** The bundled demos take no remote. */
  demo?: boolean;
}) {
  const t = useGitTransport();
  const saving = useSaving(workspace);
  const v: BarView = barView(g.state);
  const pulling = g.busy === "pull";
  if (g.state && g.state.mode !== "connected" && !demo)
    return (
      <Button
        variant="glass"
        onClick={() => openSyncSettings(workspace)}
        title={m["git.card.connectName"]({ workspace })}
        aria-label={m["git.card.connectName"]({ workspace })}
      >
        <RefreshCw className="size-5" />
        <span className="max-sm:hidden">{m["git.card.connect"]()}</span>
      </Button>
    );
  if (!v.show || !v.remote) return null;
  if (v.merge !== null)
    return (
      <Button onClick={() => openConflict(workspace)}>
        <GitMerge className="size-5" />
        {(v.merge === 1 ? m["git.mergeOpen.one"] : m["git.mergeOpen.other"])({
          n: v.merge,
        })}
      </Button>
    );
  const refused = v.pullBlocked === "commitFirst";
  const pullName =
    v.toPull > 0 ? m["git.card.pullN"]({ n: v.toPull }) : m["git.pull"]();
  const pushable = v.commit || v.push;
  const busy = !!g.busy || saving;
  // With nothing incoming and nothing to send, the pills step aside for a lasting line.
  if (!pulling && !refused && v.toPull === 0 && !pushable && v.remoteEmpty)
    return (
      <span
        role="status"
        className="type-secondary flex shrink-0 flex-col text-fg-2"
      >
        <span>{m["git.card.remoteEmpty"]({ repo: remoteName(v.remote) })}</span>
        <span className="type-status text-fg-3">
          {m["git.card.remoteEmptyHint"]()}
        </span>
      </span>
    );
  if (!pulling && !refused && v.toPull === 0 && !pushable)
    return (
      <span role="status" className="type-secondary shrink-0 text-fg-2">
        {g.state?.lastFetch?.at
          ? m["git.card.checkedAgo"]({
              when: editedAgo(g.state.lastFetch.at, Date.now(), getLocale()),
            })
          : m["git.card.upToDate"]()}
      </span>
    );
  return (
    <>
      {(pulling || refused || v.toPull > 0) && (
        <Button
          variant="glass"
          aria-label={pulling ? m["git.card.pulling"]() : pullName}
          // Said, not disabled: a disabled button shows no tooltip, and this one has a reason to give.
          aria-disabled={refused || !v.pull || busy || undefined}
          title={refused ? m["git.commitFirst"]() : undefined}
          onClick={() => {
            if (!refused && v.pull && !busy) void g.act("pull");
          }}
          className={
            refused
              ? "border border-(--k7-warn-line) text-warn"
              : !v.pull && !pulling
                ? "opacity-45"
                : undefined
          }
        >
          {pulling ? (
            <Loader2 className="size-5 animate-spin" />
          ) : (
            <ArrowDownToLine className="size-5" />
          )}
          {pulling
            ? m["git.card.pulling"]()
            : refused
              ? m["git.commitFirstShort"]()
              : m["git.pull"]()}
          {!pulling && !refused && v.toPull > 0 && (
            <Badge variant="count">{v.toPull}</Badge>
          )}
        </Button>
      )}
      {/* Save waits while an update is incoming, unless the update itself waits for a save. */}
      {(pulling || pushable) && (refused || v.toPull === 0) && (
        <Button
          variant="glass"
          aria-label={
            v.changed > 0
              ? m["git.card.saveN"]({ n: v.changed })
              : m["git.save.label"]()
          }
          aria-disabled={!pushable || busy || undefined}
          onClick={() => {
            if (pushable && !busy && t)
              void save(t, workspace, { onPull: () => void g.act("pull") });
          }}
          className={pulling || !pushable ? "opacity-45" : undefined}
        >
          {saving ? (
            <Loader2 className="size-5 animate-spin" />
          ) : (
            <Save className="size-5" />
          )}
          <span className="max-sm:hidden">{m["git.save.label"]()}</span>
          {v.changed > 0 && <Badge variant="count">{v.changed}</Badge>}
        </Button>
      )}
    </>
  );
}
