// PanelRail.tsx — one vertical toolbar rising from the chevron: the Export panel (git's Save and Pull inside) nearest it, the panes, the panels, Settings, Close on top.
// It rises from the chevron at the bottom right once unfolded; a phone has no rail, its overflow menu holds the same buttons (docs/shell.md).

import { Badge } from "@studio/ui/badge";
import { Button } from "@studio/ui/button";
import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import {
  LogOut,
  type LucideIcon,
  Settings,
  SquareTerminal,
} from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { summonChord } from "./modKey";
import { PanelButton } from "./PanelButton";
import type { HeaderPane, HeaderPanel } from "./ProjectHeader";

/** A view option of the pane, on a phone's rail (its lane views, its graph's fit): a switch, or an action. */
export interface RailView {
  id: string;
  label: string;
  icon: LucideIcon;
  on: boolean;
  /** A plain action (Zoom fit) rather than a switch: no pressed state. */
  action?: boolean;
  onPress: () => void;
}

export function PanelRail({
  views,
  share,
  unsaved = 0,
  panes,
  pane,
  onPane,
  panels,
  settingsPressed,
  onSettings,
  closeHref,
  onCommandBar,
  bottom,
}: {
  /** The pane's view options on a phone, which has no view capsule; above the panes. */
  views?: RailView[];
  /** Where it stands on a phone, whose bar is measured; a desk's sits on --k7-float-bottom. */
  bottom?: number;
  /** The Export panel, which holds git's Save and Update: last, nearest the chevron. */
  share?: HeaderPanel;
  /** The changes waiting to be saved, counted on the Export button. */
  unsaved?: number;
  panes: HeaderPane[];
  pane: string;
  onPane: (id: string) => void;
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
  /** Where Close goes: the projects list. */
  closeHref: string;
  /** The command bar's button, above git's, where the floating row is short of room (a tablet in portrait). */
  onCommandBar?: () => void;
}) {
  return (
    <Capsule
      aria-label={m["shell.panels"]()}
      aria-orientation="vertical"
      style={bottom === undefined ? undefined : { bottom }}
      className="float absolute right-4 bottom-[calc(var(--k7-float-bottom)+var(--k7-float-h)+8px)] z-(--k7-z-bar) h-auto max-h-[calc(100%-var(--k7-float-bottom)-var(--k7-float-h)-24px)] flex-col gap-1.5 overflow-x-hidden overflow-y-auto p-1 [scrollbar-width:none] desk:h-auto [&::-webkit-scrollbar]:hidden"
    >
      {/* Read from the chevron up: Export nearest it, Close at the top. */}
      {/* A link (the projects list is another page) drawn as the rail's buttons. */}
      <Button
        asChild
        variant="ghost"
        size="icon"
        className="[&_svg]:size-5 desk:[&_svg]:size-[18px]"
      >
        <a
          href={closeHref}
          aria-label={m["shell.close"]()}
          title={m["shell.close"]()}
        >
          <LogOut />
        </a>
      </Button>
      <CapsuleButton
        size="icon"
        pressed={settingsPressed}
        aria-label={m["settings.menu"]()}
        title={m["settings.menu"]()}
        onClick={onSettings}
      >
        <Settings />
      </CapsuleButton>
      <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
      {[...panels].reverse().map((p) => (
        <PanelButton key={p.id} panel={p} size="icon" />
      ))}
      <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
      {views && views.length > 0 && (
        <>
          <div
            role="group"
            aria-label={m["shell.laneViews"]()}
            className="flex flex-col gap-1.5"
          >
            {views.map((v) => (
              <CapsuleButton
                key={v.id}
                size="icon"
                pressed={v.action ? undefined : v.on}
                aria-label={v.label}
                title={v.label}
                onClick={v.onPress}
              >
                <v.icon />
              </CapsuleButton>
            ))}
          </div>
          <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
        </>
      )}
      <div
        role="group"
        aria-label={m["shell.pane"]()}
        className="flex flex-col gap-1.5"
      >
        {[...panes].reverse().map((p) => (
          <CapsuleButton
            key={p.id}
            size="icon"
            pressed={pane === p.id}
            disabled={!!p.blocked}
            disabledReason={p.blocked}
            aria-label={p.label}
            title={
              p.blocked ?? (p.shortcut ? `${p.label} (${p.shortcut})` : p.label)
            }
            onClick={() => onPane(p.id)}
          >
            <p.icon />
          </CapsuleButton>
        ))}
      </div>
      {onCommandBar && (
        <>
          <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
          <CapsuleButton
            size="icon"
            aria-label={m["shell.commandBar"]()}
            title={m["shell.commandBar.tip"]({
              name: m["commandBar.list"](),
              keys: summonChord(),
            })}
            onClick={onCommandBar}
          >
            <SquareTerminal />
          </CapsuleButton>
        </>
      )}
      {share && (
        <>
          <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
          <PanelButton panel={share} size="icon">
            {unsaved > 0 && (
              <Badge
                variant="count-sm"
                aria-hidden
                className="pointer-events-none absolute -top-1.5 -right-1.5"
              >
                {unsaved}
              </Badge>
            )}
          </PanelButton>
        </>
      )}
    </Capsule>
  );
}
