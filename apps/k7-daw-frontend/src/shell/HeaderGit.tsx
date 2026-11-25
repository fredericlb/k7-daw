// HeaderGit.tsx — the header's git capsule: Save with its count (it only pushes when only saves wait), and a menu (Pull, Saves).
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
  ChevronDown,
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

  const menu = (
    <DropdownMenuContent align="end" className="w-[260px]">
      <DropdownMenuLabel className="font-mono">{caption}</DropdownMenuLabel>
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

  return (
    <Capsule aria-label={m["git.bar"]()} className="shrink-0 desk:p-0">
      <CapsuleButton
        aria-label={commitName}
        title={commitName}
        disabled={!(v.commit || v.push) || busy}
        onClick={commit}
        className="px-3.5 desk:h-[30px] desk:rounded-[15px]"
      >
        {saving ? <Loader2 className="animate-spin" /> : <Save />}
        <span className="max-[1279px]:hidden">{commitLabel}</span>
        {v.changed > 0 && <Badge variant="count">{v.changed}</Badge>}
      </CapsuleButton>
      <CapsuleDivider />
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <CapsuleButton
            aria-label={m["shell.git.more"]()}
            className="desk:h-[30px] desk:rounded-[15px]"
          >
            <ChevronDown />
          </CapsuleButton>
        </DropdownMenuTrigger>
        {menu}
      </DropdownMenu>
    </Capsule>
  );
}
