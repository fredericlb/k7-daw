// HeaderGit.tsx — git among the floating controls: only the Resolve button while a merge waits, and the toasts a pull or a
// failure says; Save and Pull with their counts are the rail's (GitRailSave, GitRailPull).
// While a merge waits it is one accent "Resolve N conflicts" button. It decides no rule: gitState.ts's barView does.
import { showToast } from "@studio/project-shell/toast";
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { CapsuleButton } from "@studio/ui/capsule";
import { ArrowDownToLine, GitMerge, Loader2, Save } from "@studio/ui/icons";
import { useEffect } from "react";
import type { GitTransport } from "../git/gitApi";
import { openConflict } from "../git/gitScreens";
import { type BarView, barView } from "../git/gitState";
import { pulledSentence } from "../git/pulledSentence";
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
  // What a pull came to, and what failed, are said as toasts: the header holds no sentence.
  useEffect(() => {
    const said = pulledSentence(g.pulled);
    if (said) showToast({ message: said });
  }, [g.pulled]);
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

// The rail's Save: the count of changed files on its corner, the last auto-save in its tooltip; none while a merge waits (the header's Resolve stands).
export function GitRailSave({
  git: g,
  transport,
  workspace,
  className,
}: {
  git: GitHandle;
  transport: GitTransport | null;
  workspace: string;
  className: string;
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
  return (
    <CapsuleButton
      aria-label={name}
      title={autosaved ? `${name}\n${autosaved}` : name}
      disabled={!(v.commit || v.push) || busy}
      onClick={() => {
        if (!transport || busy) return;
        void save(transport, workspace, { onPull: () => void g.act("pull") });
      }}
      className={`relative ${className}`}
    >
      {saving ? <Loader2 className="animate-spin" /> : <Save />}
      {/* The tooltip's auto-save line, readable without a hover (the suite reads it here). */}
      {autosaved && (
        <span className="sr-only" data-testid="git-autosaved">
          {autosaved}
        </span>
      )}
      {v.changed > 0 && (
        <Badge
          variant="count-sm"
          aria-hidden
          className="pointer-events-none absolute -top-1.5 -right-1.5"
        >
          {v.changed}
        </Badge>
      )}
    </CapsuleButton>
  );
}

// The rail's Pull, under Save: the count of commits to fetch on its corner; dimmed with its reason while saves wait.
export function GitRailPull({
  git: g,
  workspace,
  className,
}: {
  git: GitHandle;
  workspace: string;
  className: string;
}) {
  const v = barView(g.state);
  const saving = useSaving(workspace);
  if (!v.show || resolveLabel(v) !== null) return null;
  const label = m["git.pull"]();
  return (
    <CapsuleButton
      aria-label={label}
      title={label}
      disabled={!!g.busy || saving || !v.pull}
      disabledReason={
        v.pullBlocked === "commitFirst" ? m["git.commitFirst"]() : undefined
      }
      onClick={() => void g.act("pull")}
      className={`relative ${className}`}
    >
      <ArrowDownToLine />
      {v.toPull > 0 && (
        <Badge
          variant="count-sm"
          aria-hidden
          className="pointer-events-none absolute -top-1.5 -right-1.5"
        >
          {v.toPull}
        </Badge>
      )}
    </CapsuleButton>
  );
}
