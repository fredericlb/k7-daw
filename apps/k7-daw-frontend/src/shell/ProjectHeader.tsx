// ProjectHeader.tsx — the project's header: Back, name over status, spinner, warning chip, git, the pane
// switcher (centred) and the dock's buttons; a phone folds the right half into one menu. Right panels open from PanelRail.tsx.
// The engine's state is only a data attribute here (e2e reads it): the person is told by toasts, never by a light.
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import {
  ChevronDown,
  ChevronLeft,
  ChevronUp,
  type LucideIcon,
  TriangleAlert,
} from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { type ReactNode, useSyncExternalStore } from "react";
import { StatusText } from "../components/StatusText";
import { m } from "../paraglide/messages.js";
import {
  processesSnapshot,
  runningCount,
  subscribeProcesses,
} from "../processes";
import type { createStatusLine } from "../statusLine";
import type { ShellLayout } from "../useIsMobile";
import { PANEL_DEFS, type PanelId } from "./panels";

export interface HeaderPane {
  id: string;
  label: string;
  icon: LucideIcon;
  /** The number key that opens it (keys.ts names panes, not ring positions), shown in the tooltip. */
  shortcut?: string;
  /** Why this pane cannot be opened now ("In use on <device>"): said on a press, the pane drawn dimmed. */
  blocked?: string;
}

export interface HeaderPanel {
  id: PanelId;
  pressed: boolean;
  disabled?: boolean;
  title?: string;
  onPress: () => void;
}

export function ProjectHeader({
  layout,
  name,
  backHref,
  statusStore,
  engine,
  problems,
  onProblems,
  onProcesses,
  extra,
  git,
  panes,
  pane,
  onPane,
  panels,
  rail,
  overflow,
  lockBanner,
}: {
  layout: ShellLayout;
  name: string;
  backHref: string;
  statusStore: ReturnType<typeof createStatusLine>;
  engine: "up" | "down" | "none";
  problems: number;
  onProblems: () => void;
  onProcesses: () => void;
  /** Small controls after the title, such as the bench's Save. */
  extra?: ReactNode;
  git: ReactNode;
  panes: HeaderPane[];
  pane: string;
  onPane: (id: string) => void;
  /** The DOCK's buttons (the Mixer); the right panels' are the rail's. */
  panels: HeaderPanel[];
  /** The chevron that unfolds the right-edge rail (PanelRail.tsx), last in the header; none on a phone. */
  rail?: { open: boolean; onToggle: () => void };
  /** The phone's menu, standing in for the capsules on the right. */
  overflow: ReactNode;
  /** The sentence under the header while a merge locks editing. */
  lockBanner?: string | null;
}) {
  const phone = layout === "phone";
  const all = useSyncExternalStore(subscribeProcesses, processesSnapshot);
  const running = runningCount(all);
  return (
    <>
      <header
        aria-label={m["shell.header"]()}
        data-testid="engine-status"
        data-state={engine}
        className={
          phone
            ? "relative z-(--k7-z-header) flex h-(--k7-header-phone-h) shrink-0 items-center gap-2 px-4"
            : "relative z-(--k7-z-header) flex h-(--k7-header-h) shrink-0 items-center gap-2 px-4 py-2"
        }
      >
        <div className="flex min-w-0 flex-1 basis-0 items-center gap-2">
          <Button asChild variant="glass" size="round" className="shrink-0">
            <a
              href={backHref}
              aria-label={m["shell.back"]()}
              title={m["shell.back"]()}
            >
              <ChevronLeft className="size-5" strokeWidth={2.2} />
            </a>
          </Button>
          {/* The name takes the free width, and the status line under it (a phone draws it under the header). */}
          <div className="min-w-0 shrink">
            <h1 className="type-headline truncate text-foreground" title={name}>
              {name || "…"}
            </h1>
            {!phone && (
              <p className="type-status truncate text-fg-2">
                <StatusText store={statusStore} />
              </p>
            )}
          </div>
          {running > 0 && (
            <button
              type="button"
              aria-label={m["shell.processes"]({ n: running })}
              title={m["shell.processes"]({ n: running })}
              onClick={onProcesses}
              className="grid size-11 shrink-0 cursor-pointer desk:size-8 place-items-center rounded-full outline-none focus-visible:shadow-(--k7-focus-ring)"
            >
              <Spinner />
            </button>
          )}
          {problems > 0 && (
            <Badge asChild variant="warn">
              <button
                type="button"
                aria-label={m["shell.problems"]({ n: problems })}
                title={m["shell.problems"]({ n: problems })}
                onClick={onProblems}
                className={
                  phone
                    ? undefined
                    : "relative before:absolute before:-inset-2 before:content-['']"
                }
              >
                <TriangleAlert className="size-4" />
                {problems}
              </button>
            </Badge>
          )}
          {extra}
          {git}
        </div>
        {phone ? (
          overflow
        ) : (
          <>
            <Capsule aria-label={m["shell.pane"]()} className="shrink-0">
              {panes.map((p) => (
                <CapsuleButton
                  key={p.id}
                  pressed={pane === p.id}
                  disabled={!!p.blocked}
                  disabledReason={p.blocked}
                  aria-label={p.label}
                  title={
                    p.blocked ??
                    (p.shortcut ? `${p.label} (${p.shortcut})` : p.label)
                  }
                  onClick={() => onPane(p.id)}
                >
                  <p.icon />
                  {pane === p.id && <span>{p.label}</span>}
                </CapsuleButton>
              ))}
            </Capsule>
            <div className="flex min-w-max flex-1 basis-0 items-center justify-end gap-2">
              {/* The dock's own buttons stay here; the right panels' are the rail's (PanelRail.tsx). */}
              <Capsule role="group" aria-label={m["shell.dock"]()}>
                {panels.map((p) => {
                  const def = PANEL_DEFS[p.id];
                  return (
                    <CapsuleButton
                      key={p.id}
                      pressed={p.pressed}
                      disabled={p.disabled}
                      disabledReason={p.title}
                      aria-label={def.title()}
                      title={p.title ?? def.title()}
                      onClick={p.onPress}
                      className={p.pressed ? "gap-1.5" : undefined}
                    >
                      <def.icon />
                      {p.pressed && <span>{def.title()}</span>}
                    </CapsuleButton>
                  );
                })}
              </Capsule>
              {rail && (
                <Capsule>
                  <CapsuleButton
                    aria-expanded={rail.open}
                    aria-label={
                      rail.open
                        ? m["shell.panelsHide"]()
                        : m["shell.panelsShow"]()
                    }
                    title={
                      rail.open
                        ? m["shell.panelsHide"]()
                        : m["shell.panelsShow"]()
                    }
                    onClick={rail.onToggle}
                  >
                    {rail.open ? <ChevronUp /> : <ChevronDown />}
                  </CapsuleButton>
                </Capsule>
              )}
            </div>
          </>
        )}
      </header>
      {phone && (
        <p className="type-status mx-4 mb-1 shrink-0 text-fg-2 empty:hidden">
          <StatusText store={statusStore} />
        </p>
      )}
      {lockBanner && (
        <p
          role="status"
          className="type-secondary mx-4 mb-1 shrink-0 rounded-2xl border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3.5 py-2 text-warn"
        >
          {lockBanner}
        </p>
      )}
    </>
  );
}
