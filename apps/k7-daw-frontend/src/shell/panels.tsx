// panels.tsx — the registry of the shell's panels: an id, a glyph and a title each, and the frame they open in.
// The frame is the kit's SidePanel (Pad, Desk) or Sheet (Phone); what goes inside is each panel's own component.
// `references` and `history` are segments of Resources and Tasks: they have no button, only the shared one.
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
import type { Panel } from "../routes";
import type { RightPanelId } from "../ui/commands/context";

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

/** The legacy bottom panel (`?panel=`) as the right panel that now holds it. */
const OF_LEGACY: Record<Panel, PanelId> = {
  event: "events",
  graph: "chain",
  mix: "mixer",
  logs: "logs",
};

/** The one panel on screen: the URL-borne one wins, there being never both (App keeps them exclusive). */
export function shownPanel(
  legacy: Panel | null,
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
  return (
    <Frame
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
