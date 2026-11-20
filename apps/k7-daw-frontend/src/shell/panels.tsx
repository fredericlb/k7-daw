// panels.tsx — the registry of the shell's panels: an id, a glyph and a title each, and the frame they open in.
// The frame is the kit's SidePanel (Pad, Desk) or Sheet (Phone); what goes inside is each panel's own component.
// `references` and `history` are segments of Resources and Tasks: they have no button, only the shared one.
// `events`, `mixer` and `chain` are the DOCK's occupants, not right panels (docs/shell.md).

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

/** What the bottom dock holds: each is named and glyphed here, and none is a right panel. */
export type DockPanel = "events" | "mixer" | "chain";

/** The panels drawn in the right column. */
export type RightPanel = Exclude<PanelId, DockPanel>;

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

/** The dock's occupant that each URL `?panel=` value names; `logs` is a right panel. */
export const DOCK_OF_PANEL: Record<Exclude<Panel, "logs">, DockPanel> = {
  event: "events",
  mix: "mixer",
  graph: "chain",
};

/** The `?panel=` value an occupant rides as. */
export const PANEL_OF_DOCK: Record<DockPanel, Panel> = {
  events: "event",
  mixer: "mix",
  chain: "graph",
};

/** The one right panel on screen: the URL-borne one wins, there being never both (App keeps them exclusive). */
export function shownPanel(
  legacy: "logs" | null,
  right: RightPanelId | null,
): RightPanel | null {
  if (legacy) return legacy;
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
  id: RightPanel;
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

/** The dock, holding one occupant: a bottom dock on Pad and Desk, a partial sheet on a phone. */
export function DockFrame({
  occupant,
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
  occupant: DockPanel;
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
  const def = PANEL_DEFS[occupant];
  // The ladder's Escape leaves the expanded state first, as the dock's own Escape does.
  usePopin(true, () => (expanded ? onExpandedChange(false) : onClose()));
  const common = {
    title: def.title(),
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
