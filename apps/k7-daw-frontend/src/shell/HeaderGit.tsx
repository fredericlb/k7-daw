// HeaderGit.tsx — git in the header: on a phone a menu (Save, Pull, Saves) titled by the branch and the last auto-save; on a
// desk only the Resolve button while a merge waits, Save with its count being GitRailSave, the rail's first button.
// While a merge waits it is one accent "Resolve N conflicts" button. It decides no rule: gitState.ts's barView does.
import { showToast } from "@studio/project-shell/toast";
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { CapsuleButton } from "@studio/ui/capsule";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  ArrowDownToLine,
  GitMerge,
  History,
  Loader2,
  Save,
} from "@studio/ui/icons";
import { useEffect } from "react";
import type { GitTransport } from "../git/gitApi";
import { openConflict } from "../git/gitScreens";
import { type BarView, barView } from "../git/gitState";
import type { PullState } from "../git/gitTypes";
import { save, useSaving } from "../git/saveAct";
import type { GitHandle } from "../git/useGitState";
import { m } from "../paraglide/messages.js";

const pulledSentence = (p: PullState | null): string | null =>
  p === "up-to-date"
    ? m["git.pulledNothing"]()
    : p === "empty-remote"
      ? m["git.emptyRemote"]()
      : p === "merged"
        ? m["git.pulledMerged"]()
        : null;

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
  transport,
  workspace,
  phone,
  onCommits,
}: {
  git: GitHandle;
  transport: GitTransport | null;
  workspace: string;
  phone: boolean;
  /** Open the Saves list (the History panel). */
  onCommits: () => void;
}) {
  const v = barView(g.state);
  const saving = useSaving(workspace);
  const busy = !!g.busy || saving;
  const commit = () => {
    if (!transport || busy) return;
    void save(transport, workspace, { onPull: () => void g.act("pull") });
  };
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
  const commitLabel = m["git.save.label"]();
  const commitName =
    v.changed > 0
      ? m["git.save.n"]({ n: v.changed })
      : v.push
        ? commitLabel
        : m["git.save.nothing"]();
  const caption = v.remote
    ? m["git.caption"]({ branch: v.branch, remote: "origin" })
    : m["git.headlineLocal"]({ branch: v.branch });
  const pullDisabled = busy || !v.pull;
  const autosaved = autosaveLine(v);

  const menu = (
    <DropdownMenuContent align="end" className="w-[260px]">
      <DropdownMenuLabel className="font-mono">{caption}</DropdownMenuLabel>
      {autosaved && (
        <DropdownMenuLabel
          data-testid="git-autosaved"
          className="pt-0 font-normal text-muted-foreground text-xs"
        >
          {autosaved}
        </DropdownMenuLabel>
      )}
      {phone && (
        <DropdownMenuItem
          disabled={!(v.commit || v.push) || busy}
          onSelect={commit}
        >
          <Save className="size-5 text-primary-text" />
          {commitLabel}
          {v.changed > 0 && <Badge variant="count">{v.changed}</Badge>}
        </DropdownMenuItem>
      )}
      <DropdownMenuItem
        disabled={pullDisabled}
        disabledReason={
          v.pullBlocked === "commitFirst" ? m["git.commitFirst"]() : undefined
        }
        onSelect={() => void g.act("pull")}
      >
        <ArrowDownToLine className="size-5 text-primary-text" />
        {m["git.pull"]()}
        {v.toPull > 0 && <Badge variant="count">{v.toPull}</Badge>}
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={onCommits}>
        <History className="size-5 text-primary-text" />
        {m["shell.git.commits"]()}
      </DropdownMenuItem>
    </DropdownMenuContent>
  );

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

  if (phone)
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="glass"
            size="round"
            className="relative shrink-0"
            aria-label={m["shell.git.phoneName"]({
              commit: commitName,
              pull: v.toPull,
            })}
          >
            <Save className="size-5" />
            {v.changed + v.toPull > 0 && (
              <Badge
                variant="count-sm"
                className="absolute -top-1 -right-1"
                aria-hidden
              >
                {v.changed + v.toPull}
              </Badge>
            )}
          </Button>
        </DropdownMenuTrigger>
        {menu}
      </DropdownMenu>
    );

  // A desk's header holds no git button: Save is the rail's (GitRailSave), the Saves list is Tasks & history.
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
