// HeaderGit.tsx — the header's git capsule: Commit & Push with its count, and a menu (Pull, Commits).
// While a merge waits it is one accent "Resolve N conflicts" button. It decides no rule: gitState.ts's barView does.
import { showToast } from "@studio/project-shell/toast";
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  GitCommitHorizontal,
  GitMerge,
  History,
} from "@studio/ui/icons";
import { useEffect } from "react";
import { openCommitAndPush, openConflict } from "../git/gitScreens";
import { type BarView, barView } from "../git/gitState";
import type { PullState } from "../git/gitTypes";
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
  onCommits,
}: {
  git: GitHandle;
  workspace: string;
  phone: boolean;
  /** Open the Commits list (the History panel). */
  onCommits: () => void;
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
  const commitLabel = m["shell.git.commitPush"]();
  const commitName =
    v.changed > 0 ? m["shell.git.commitPushN"]({ n: v.changed }) : commitLabel;
  const caption = v.remote
    ? m["git.headline"]({ remote: "origin", branch: v.branch })
    : m["git.headlineLocal"]({ branch: v.branch });
  const pullDisabled = !!g.busy || !v.pull;

  const menu = (
    <DropdownMenuContent align="end" className="w-[260px]">
      <DropdownMenuLabel className="font-mono">{caption}</DropdownMenuLabel>
      {phone && (
        <DropdownMenuItem
          disabled={!v.commit || !!g.busy}
          onSelect={() => openCommitAndPush(workspace)}
        >
          <GitCommitHorizontal className="size-5 text-primary-text" />
          {commitLabel}
          {v.changed > 0 && <Badge variant="count">{v.changed}</Badge>}
        </DropdownMenuItem>
      )}
      <DropdownMenuItem
        disabled={pullDisabled}
        title={
          v.pullBlocked === "commitFirst" ? m["git.commitFirst"]() : undefined
        }
        onSelect={() => void g.act("pull")}
      >
        <ArrowDownToLine className="size-5 text-primary-text" />
        {m["git.pull"]()}
        {v.toPull > 0 && <Badge variant="count">{v.toPull}</Badge>}
      </DropdownMenuItem>
      <DropdownMenuItem
        disabled={!v.push || !!g.busy}
        onSelect={() => void g.act("push")}
      >
        <ArrowUpFromLine className="size-5 text-primary-text" />
        {m["git.push"]()}
        {v.toPush > 0 && <Badge variant="count">{v.toPush}</Badge>}
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
            <GitCommitHorizontal className="size-5" />
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

  return (
    <Capsule aria-label={m["git.bar"]()} className="shrink-0">
      <CapsuleButton
        aria-label={commitName}
        disabled={!v.commit || !!g.busy}
        onClick={() => openCommitAndPush(workspace)}
        className="px-3.5"
      >
        <GitCommitHorizontal />
        <span className="max-[1279px]:hidden">{commitLabel}</span>
        {v.changed > 0 && <Badge variant="count">{v.changed}</Badge>}
      </CapsuleButton>
      <CapsuleDivider />
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <CapsuleButton aria-label={m["shell.git.more"]()}>
            <ChevronDown />
          </CapsuleButton>
        </DropdownMenuTrigger>
        {menu}
      </DropdownMenu>
    </Capsule>
  );
}
