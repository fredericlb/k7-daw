// ProjectHeader.tsx — the project's header: Back, name over status, spinner, warning chip, git, the pane
// switcher (centred), the panel toolbar and the app buttons; a phone folds the right half into one menu.
// The engine's state is only a data attribute here (e2e reads it): the person is told by toasts, never by a light.
import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import {
  ChevronLeft,
  Keyboard,
  type LucideIcon,
  Settings,
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
  settingsPressed,
  onSettings,
  onKeyboard,
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
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
  onKeyboard: () => void;
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
        <Button asChild variant="glass" size="round" className="shrink-0">
          <a
            href={backHref}
            aria-label={m["shell.back"]()}
            title={m["shell.back"]()}
          >
            <ChevronLeft className="size-5" strokeWidth={2.2} />
          </a>
        </Button>
        <div className="min-w-0 max-w-[160px] shrink">
          <h1 className="type-headline truncate text-foreground">
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
            // biome-ignore lint/a11y/useSemanticElements: a link by name for e2e and readers; it opens a panel, with no address of its own.
            role="link"
            aria-label={m["shell.processes"]({ n: running })}
            title={m["shell.processes"]({ n: running })}
            onClick={onProcesses}
            className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full outline-none focus-visible:shadow-(--k7-focus-ring)"
          >
            <Spinner />
          </button>
        )}
        {problems > 0 && (
          <Badge asChild variant="warn">
            <button
              type="button"
              // biome-ignore lint/a11y/useSemanticElements: a link by name for e2e and readers; it opens a panel, with no address of its own.
              role="link"
              aria-label={m["shell.problems"]({ n: problems })}
              title={m["shell.problems"]({ n: problems })}
              onClick={onProblems}
            >
              <TriangleAlert className="size-4" />
              {problems}
            </button>
          </Badge>
        )}
        {extra}
        {git}
        {phone ? (
          <>
            <span className="flex-1" />
            {overflow}
          </>
        ) : (
          <>
            <Capsule
              aria-label={m["shell.pane"]()}
              className="-translate-x-1/2 absolute left-1/2"
            >
              {panes.map((p) => (
                <CapsuleButton
                  key={p.id}
                  pressed={pane === p.id}
                  aria-label={p.label}
                  title={p.label}
                  onClick={() => onPane(p.id)}
                >
                  <p.icon />
                  {pane === p.id && <span>{p.label}</span>}
                </CapsuleButton>
              ))}
            </Capsule>
            <span className="flex-1" />
            <Capsule role="toolbar" aria-label={m["shell.panels"]()}>
              {panels.map((p) => {
                const def = PANEL_DEFS[p.id];
                return (
                  <CapsuleButton
                    key={p.id}
                    pressed={p.pressed}
                    disabled={p.disabled}
                    aria-label={def.title()}
                    title={p.title ?? def.title()}
                    onClick={p.onPress}
                  >
                    <def.icon />
                  </CapsuleButton>
                );
              })}
            </Capsule>
            <Capsule role="toolbar" aria-label={m["shell.app"]()}>
              <CapsuleButton
                pressed={settingsPressed}
                aria-label={m["settings.menu"]()}
                title={m["settings.menu"]()}
                onClick={onSettings}
              >
                <Settings />
              </CapsuleButton>
              <CapsuleButton
                aria-label={m["shell.keyboard"]()}
                title={m["shell.keyboard"]()}
                onClick={onKeyboard}
              >
                <Keyboard />
              </CapsuleButton>
            </Capsule>
          </>
        )}
      </header>
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
