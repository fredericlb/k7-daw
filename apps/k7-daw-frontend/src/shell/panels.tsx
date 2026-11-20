// panels.tsx — the registry of the shell's panels: an id, a glyph and a title each, and the frame they open in.
// The frame is the kit's SidePanel (Pad, Desk) or Sheet (Phone); what goes inside is each panel's own component.
// `references` and `history` are segments of Resources and Tasks: they have no button, only the shared one.

import { EditorDock, EditorSheet } from "@studio/ui/editor-dock";
import {
  FolderOpen,
  ListChecks,
  ListMusic,
  type LucideIcon,
  Music,
  ScrollText,
  Settings,
  Share2,
  SlidersVertical,
  Workflow,
} from "@studio/ui/icons";
import { Sheet, SidePanel } from "@studio/ui/side-panel";
import type { ReactNode } from "react";
import { m } from "../paraglide/messages.js";
import { usePopin } from "../popins";
import type { Panel } from "../routes";
import type { RightPanelId } from "../ui/commands/context";
import type { ShellLayout } from "../useIsMobile";

export type PanelId =
  | "resources"
  | "playlists"
  | "mixer"
  | "events"
  | "logs"
  | "tasks"
  | "export"
  | "settings"
  | "chain"
  | "references"
  | "history";

/** The panels with a button in the header's toolbar, in its order. */
export const TOOLBAR_PANELS = [
  "resources",
  "playlists",
  "mixer",
  "events",
  "logs",
  "tasks",
  "export",
] as const satisfies readonly PanelId[];

export interface PanelDef {
  icon: LucideIcon;
  title: () => string;
  /** The toolbar button that stands for this panel while it is open. */
  button: PanelId;
}

export const PANEL_DEFS: Record<PanelId, PanelDef> = {
  resources: {
    icon: FolderOpen,
    title: () => m["shell.panel.resources"](),
    button: "resources",
  },
  playlists: {
    icon: ListMusic,
    title: () => m["shell.panel.playlists"](),
    button: "playlists",
  },
  mixer: {
    icon: SlidersVertical,
    title: () => m["shell.panel.mixer"](),
    button: "mixer",
  },
  events: {
    icon: Music,
    title: () => m["shell.panel.events"](),
    button: "events",
  },
  logs: {
    icon: ScrollText,
    title: () => m["shell.panel.logs"](),
    button: "logs",
  },
  tasks: {
    icon: ListChecks,
    title: () => m["shell.panel.tasks"](),
    button: "tasks",
  },
  export: {
    icon: Share2,
    title: () => m["shell.panel.export"](),
    button: "export",
  },
  settings: {
    icon: Settings,
    title: () => m["shell.panel.settings"](),
    button: "settings",
  },
  chain: {
    icon: Workflow,
    title: () => m["shell.panel.chain"](),
    button: "chain",
  },
  references: {
    icon: FolderOpen,
    title: () => m["shell.panel.resources"](),
    button: "resources",
  },
  history: {
    icon: ListChecks,
    title: () => m["shell.panel.tasks"](),
    button: "tasks",
  },
};

/** The legacy bottom panel (`?panel=`) as the right panel that now holds it; `event` is the dock, not a right panel. */
const OF_LEGACY: Record<Exclude<Panel, "event">, PanelId> = {
  graph: "chain",
  mix: "mixer",
  logs: "logs",
};

/** The one panel on screen: the URL-borne one wins, there being never both (App keeps them exclusive). */
export function shownPanel(
  legacy: Exclude<Panel, "event"> | null,
  right: RightPanelId | null,
): PanelId | null {
  if (legacy) return OF_LEGACY[legacy];
  if (right && right !== "comments") return right;
  return null;
}

/** The panel's frame: a floating side panel, or on a phone a full-screen sheet. */
export function PanelFrame({
  id,
  phone,
  onClose,
  subtitle,
  actions,
  children,
}: {
  id: PanelId;
  phone: boolean;
  onClose: () => void;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const def = PANEL_DEFS[id];
  const Frame = phone ? Sheet : SidePanel;
  // Registered so an Escape pressed outside the panel closes it through the ladder (popins.ts).
  usePopin(true, onClose);
  return (
    <Frame
      onEscapeKeyDown={(e) => {
        // Inside the panel it closes itself; elsewhere the ladder owns the key and picks the front layer.
        const t = e.target as Element | null;
        if (!t?.closest?.("[data-slot=panel-frame]")) e.preventDefault();
      }}
      title={def.title()}
      icon={<def.icon aria-hidden />}
      subtitle={subtitle}
      actions={actions}
      onClose={onClose}
      closeLabel={m["shell.panel.close"]()}
    >
      {children}
    </Frame>
  );
}

/** The editor dock, which is the Events panel: a bottom dock on Pad and Desk, a partial sheet on a phone. */
export function DockFrame({
  layout,
  height,
  onHeightChange,
  expanded,
  onExpandedChange,
  onClose,
  subtitle,
  actions,
  children,
}: {
  layout: ShellLayout;
  height: number;
  onHeightChange: (h: number) => void;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  onClose: () => void;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const def = PANEL_DEFS.events;
  // The ladder's Escape leaves the expanded state first, as the dock's own Escape does.
  usePopin(true, () => (expanded ? onExpandedChange(false) : onClose()));
  const common = {
    title: def.title(),
    icon: <def.icon aria-hidden />,
    subtitle,
    actions,
    expanded,
    onExpandedChange,
    expandLabel: m["shell.dock.expand"](),
    onClose,
    closeLabel: m["shell.dock.close"](),
    onEscapeKeyDown: (e: KeyboardEvent) => {
      const t = e.target as Element | null;
      if (!t?.closest?.("[data-slot=panel-frame]")) e.preventDefault();
    },
    children,
  };
  return layout === "phone" ? (
    <EditorSheet {...common} />
  ) : (
    <EditorDock
      {...common}
      height={height}
      onHeightChange={onHeightChange}
      resizeLabel={m["shell.dock.resize"]()}
    />
  );
}
