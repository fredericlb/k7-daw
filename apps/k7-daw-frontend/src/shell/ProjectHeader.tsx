// ProjectHeader.tsx — no header row: the floating controls. A desk's views, command bar, Mixer and rail's chevron sit at
// the bottom right, its running spinner at the top right; a phone's spinner and Resolve at its top right. Panes, panels, Close and git are PanelRail.tsx's.
// The engine's state is only a data attribute here (e2e reads it): the person is told by toasts, never by a light.
import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import { type LucideIcon, SquareTerminal } from "@studio/ui/icons";
import type { ReactNode } from "react";
import { StatusText } from "../components/StatusText";
import { m } from "../paraglide/messages.js";
import type { createStatusLine } from "../statusLine";
import type { ShellLayout } from "../useIsMobile";
import { FLOAT_BUTTON, FLOAT_CAPSULE } from "./floatCapsule";
import { summonChord } from "./modKey";
import { PANEL_DEFS, type PanelId } from "./panels";
import { RailChevron } from "./RailChevron";
import { RunningBubble } from "./RunningBubble";

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
  statusStore,
  engine,
  problems,
  extra,
  git,
  panels,
  rail,
  views,
  onCommandBar,
  lockBanner,
}: {
  layout: ShellLayout;
  statusStore: ReturnType<typeof createStatusLine>;
  engine: "up" | "down" | "none";
  problems: number;
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
  /** The sentence under the header while a merge locks editing. */
  lockBanner?: string | null;
}) {
  const phone = layout === "phone";
  // A desk or a pad has no header row: its controls float at the bottom right, above the dock when it is open.
  // No header row anywhere: the controls float. A desk's sit at the bottom right; a phone's bar holds its chevron, so the
  // spinner and Resolve float at its top right instead.
  return (
    <>
      <div
        role="group"
        aria-label={m["shell.header"]()}
        data-testid="engine-status"
        data-state={engine}
        className={
          phone
            ? "pointer-events-none absolute top-3 right-3 z-(--k7-z-bar) flex items-center gap-2 *:pointer-events-auto"
            : "pointer-events-none absolute right-4 bottom-(--k7-float-bottom) z-(--k7-z-bar) flex h-(--k7-float-h) items-center gap-2 transition-[bottom] duration-(--k7-dur-panel) *:pointer-events-auto"
        }
      >
        {phone && <RunningBubble />}
        {extra}
        {git}
        {!phone && views}
        {!phone && onCommandBar && (
          <Capsule className={FLOAT_CAPSULE}>
            <CapsuleButton
              className={FLOAT_BUTTON}
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
        {!phone && (
          <Capsule
            role="group"
            aria-label={m["shell.dock"]()}
            className={FLOAT_CAPSULE}
          >
            {panels.map((p) => {
              const def = PANEL_DEFS[p.id];
              return (
                <CapsuleButton
                  className={FLOAT_BUTTON}
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
        )}
        {!phone && rail && (
          <RailChevron
            open={rail.open}
            onToggle={rail.onToggle}
            unsaved={rail.unsaved}
            problems={problems}
          />
        )}
      </div>
      {/* What runs, at the app's top right on a desk and a pad: a phone's sits in its own top-right group above. */}
      {!phone && (
        <div className="absolute top-4 right-4 z-(--k7-z-bar)">
          <RunningBubble />
        </div>
      )}
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
}
