// HeaderGit.tsx — git among the floating controls: only the Resolve button while a merge waits, and the toasts a pull or a
// failure says; Save and Pull with their counts are the Export panel's rows (GitShareRows).
// While a merge waits it is one accent "Resolve N conflicts" button. It decides no rule: gitState.ts's barView does.

import { showToast } from "@studio/project-shell/toast";
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { ArrowDownToLine, GitMerge, Save } from "@studio/ui/icons";
import { RowButton, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useEffect } from "react";
import type { GitTransport } from "../git/gitApi";
import { openConflict } from "../git/gitScreens";
import { type BarView, barView } from "../git/gitState";
import { save, useSaving } from "../git/saveAct";
import type { GitHandle } from "../git/useGitState";
import { m } from "../paraglide/messages.js";

/** When the technical branch was last written, as the menu's second title line; null before the first auto-save. */
const autosaveLine = (v: BarView): string | null => {
  if (!v.autosave) return null;
  const time = new Date(v.autosave.at).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
  return (v.autosave.unsent ? m["git.autosaveUnsent"] : m["git.autosaved"])({
    time,
  });
};

/** The merge's label and its screen, or null when no merge waits. */
const resolveLabel = (v: BarView): string | null =>
  v.merge === null
    ? null
    : (v.merge === 1 ? m["git.mergeOpen.one"] : m["git.mergeOpen.other"])({
        n: v.merge,
      });

export function HeaderGit({
  git: g,
  workspace,
  phone,
}: {
  git: GitHandle;
  workspace: string;
  phone: boolean;
}) {
  const v = barView(g.state);
  // A pull that diverged opens the conflict screen at once: the person was waiting for it.
  useEffect(() => {
    if (g.pulled === "conflicts") openConflict(workspace);
  }, [g.pulled, workspace]);
  // What failed is said as a toast; a pull that went through is seen in the files (docs/toast.md).
  useEffect(() => {
    if (g.failure) showToast({ message: g.failure, variant: "error" });
  }, [g.failure]);
  if (!v.show) return null;

  const resolve = resolveLabel(v);
  if (resolve !== null)
    return (
      <Button
        size="default"
        className="shrink-0"
        onClick={() => openConflict(workspace)}
      >
        <GitMerge className="size-5" />
        {phone ? v.merge : resolve}
        {phone && <span className="sr-only">{resolve}</span>}
      </Button>
    );

  // No git button floats otherwise: Save and Pull are the rail's, the Saves list is Tasks & history.
  return null;
}

/** What is waiting to be saved, for the badges: the changed files' count. */
export function unsavedCount(g: GitHandle): number {
  const v = barView(g.state);
  return v.show && v.merge === null ? v.changed : 0;
}

// The Export panel's git rows: Save (commit and push) with its count and last auto-save, then Update (pull) with what waits.
export function GitShareRows({
  git: g,
  transport,
  workspace,
}: {
  git: GitHandle;
  transport: GitTransport | null;
  workspace: string;
}) {
  const v = barView(g.state);
  const saving = useSaving(workspace);
  const busy = !!g.busy || saving;
  if (!v.show || resolveLabel(v) !== null) return null;
  const name =
    v.changed > 0
      ? m["git.save.n"]({ n: v.changed })
      : v.push
        ? m["git.save.label"]()
        : m["git.save.nothing"]();
  const autosaved = autosaveLine(v);
  const blocked =
    v.pullBlocked === "commitFirst" ? m["git.commitFirst"]() : null;
  return (
    <RowGroup>
      <RowButton
        tall
        icon={saving ? <Spinner inherit /> : <Save />}
        aria-label={name}
        detail={
          autosaved ? (
            <span data-testid="git-autosaved">{autosaved}</span>
          ) : undefined
        }
        disabled={!(v.commit || v.push) || busy}
        onClick={() => {
          if (!transport || busy) return;
          void save(transport, workspace, { onPull: () => void g.act("pull") });
        }}
      >
        {name}
      </RowButton>
      <RowButton
        tall
        icon={<ArrowDownToLine />}
        aria-label={m["git.pull"]()}
        detail={
          blocked ??
          (v.toPull > 0 ? m["git.toPull"]({ n: v.toPull }) : undefined)
        }
        trailing={
          v.toPull > 0 ? (
            <Badge variant="count-sm">{v.toPull}</Badge>
          ) : undefined
        }
        disabled={!!g.busy || saving || !v.pull}
        onClick={() => void g.act("pull")}
      >
        {m["git.pull"]()}
      </RowButton>
    </RowGroup>
  );
}
