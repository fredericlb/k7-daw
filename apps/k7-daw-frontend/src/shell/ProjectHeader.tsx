// ProjectHeader.tsx — a phone's header; on a desk or a pad no row: spinner, views, command bar, Mixer and rail's chevron floating.
// A phone keeps Back, the name and the warning chip (it has no rail); the panes, panels and Close are PanelRail.tsx's.
// The engine's state is only a data attribute here (e2e reads it): the person is told by toasts, never by a light.
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import {
  ChevronLeft,
  type LucideIcon,
  SquareTerminal,
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
import { summonChord } from "./modKey";
import { PANEL_DEFS, type PanelId } from "./panels";
import { RailChevron } from "./RailChevron";

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
  /** A warning count on its corner (Logs: the project's problems). */
  badge?: number;
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
  panels,
  rail,
  views,
  onCommandBar,
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
  /** The DOCK's buttons (the Mixer); the right panels' are the rail's. */
  panels: HeaderPanel[];
  /** The chevron that unfolds the right-edge rail upward (PanelRail.tsx), last of the floating controls; none on a phone. */
  rail?: { open: boolean; onToggle: () => void; unsaved: number };
  /** The pane's view options (ViewCapsule.tsx), first of the floating controls on a desk. */
  views?: ReactNode;
  /** Opens the command bar: a desk's button stands beside the Mixer, a phone's in its floating bar. */
  onCommandBar?: () => void;
  /** The phone's menu, standing in for the capsules on the right. */
  overflow: ReactNode;
  /** The sentence under the header while a merge locks editing. */
  lockBanner?: string | null;
}) {
  const phone = layout === "phone";
  const all = useSyncExternalStore(subscribeProcesses, processesSnapshot);
  const running = runningCount(all);
  // A desk or a pad has no header row: its controls float at the bottom right, above the dock when it is open.
  if (!phone)
    return (
      <>
        <div
          role="group"
          aria-label={m["shell.header"]()}
          data-testid="engine-status"
          data-state={engine}
          className="pointer-events-none absolute right-4 bottom-(--k7-float-bottom) z-(--k7-z-bar) flex h-(--k7-float-h) items-center gap-2 transition-[bottom] duration-(--k7-dur-panel) *:pointer-events-auto"
        >
          {running > 0 && (
            <button
              type="button"
              aria-label={m["shell.processes"]({ n: running })}
              title={m["shell.processes"]({ n: running })}
              onClick={onProcesses}
              className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-full outline-none focus-visible:shadow-(--k7-focus-ring)"
            >
              <Spinner />
            </button>
          )}
          {extra}
          {git}
          {views}
          {onCommandBar && (
            <Capsule className="float">
              <CapsuleButton
                aria-label={m["shell.commandBar"]()}
                title={m["shell.commandBar.tip"]({
                  name: m["commandBar.list"](),
                  keys: summonChord(),
                })}
                onClick={onCommandBar}
              >
                <SquareTerminal />
              </CapsuleButton>
            </Capsule>
          )}
          {/* The dock's own buttons stay here; the right panels' are the rail's (PanelRail.tsx). */}
          <Capsule
            role="group"
            aria-label={m["shell.dock"]()}
            className="float"
          >
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
                >
                  <def.icon />
                </CapsuleButton>
              );
            })}
          </Capsule>
          {rail && (
            <RailChevron
              open={rail.open}
              onToggle={rail.onToggle}
              unsaved={rail.unsaved}
              problems={problems}
            />
          )}
        </div>
        {/* A desk draws no status line: it is read out, not shown (docs/shell.md). */}
        <p className="sr-only">
          <StatusText store={statusStore} />
        </p>
        {lockBanner && (
          <p
            role="status"
            className="type-secondary absolute top-4 left-1/2 z-(--k7-z-bar) -translate-x-1/2 rounded-2xl border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3.5 py-2 text-warn"
          >
            {lockBanner}
          </p>
        )}
      </>
    );
  return (
    <>
      <header
        aria-label={m["shell.header"]()}
        data-testid="engine-status"
        data-state={engine}
        className="relative z-(--k7-z-header) flex h-(--k7-header-phone-h) shrink-0 items-center gap-2 px-4"
      >
        <div className="flex min-w-0 flex-1 basis-0 items-center gap-2">
          {phone && (
            <Button asChild variant="glass" size="round" className="shrink-0">
              <a
                href={backHref}
                aria-label={m["shell.back"]()}
                title={m["shell.back"]()}
              >
                <ChevronLeft className="size-5" strokeWidth={2.2} />
              </a>
            </Button>
          )}
          {/* A phone keeps the name: it has no rail and no Close. */}
          <div className="min-w-0 shrink">
            {phone && (
              <h1
                className="type-headline truncate text-foreground"
                title={name}
              >
                {name || "…"}
              </h1>
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
          {phone && problems > 0 && (
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
        {overflow}
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
