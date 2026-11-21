// SettingsPanel.tsx — the cog, as a PANEL docked at the right: one tab per settings screen,
// beside the panes rather than covering what it configures.
//
// Each tab is a COMMAND the shell dispatches (`ui settings io`, …), not a callback — that is
// what keeps the palette and the panel from drifting. COMMANDS stays a full page, reached by a
// row of the Editor tab: a sixty-row catalogue does not fit 24rem.
//
// Why several panels rather than one set of tabs, and why none of them is a pane:
// ../docs/shell.md, "The right column".

import type { EngineClient } from "@studio/engine-browser";
import { Choice } from "@studio/ui/choice";
import {
  AudioLines,
  FolderCog,
  type Settings,
  SlidersHorizontal,
} from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { EditorSettingsPage } from "./EditorSettingsPage";
import { IoPanel } from "./IoPanel";
import { ProjectSettingsTab } from "./ProjectSettingsTab";

/** The tabs, in the order they are drawn — `commands` is a PAGE and `playlists` its own panel: settings are set and left, a set list is worked from. */
export const SETTINGS_TABS = ["editor", "io", "project"] as const;

/** `account` has no tab any more: the command that names it stays typed until the command table drops it. */
export type SettingsTab = (typeof SETTINGS_TABS)[number];

const TAB_META: Record<
  (typeof SETTINGS_TABS)[number],
  { icon: typeof Settings; label: string; command: string }
> = {
  editor: {
    icon: SlidersHorizontal,
    label: m["settings.editor"](),
    command: "ui settings editor",
  },
  io: {
    icon: AudioLines,
    label: m["settings.io"](),
    command: "ui settings io",
  },
  // What the open song says about its whole mix — apps/k7-daw-frontend/packages/song/docs/console.md.
  project: {
    icon: FolderCog,
    label: m["settings.project"](),
    command: "ui settings project",
  },
};

export function SettingsPanel({
  tab,
  engine,
  vimMode,
  onRun,
}: {
  /** Which tab is showing. Owned by `App`, because a command line can change it. */
  tab: SettingsTab;
  /** For the I/O tab's meters. Null when the engine is not connected: the list still shows. */
  engine: EngineClient | null;
  vimMode: boolean;
  onRun: (line: string) => Promise<void>;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* The tabs each dispatch their own `ui settings <tab>` rather than setting state here, so the panel and the palette are one road. */}
      <div className="flex shrink-0 items-center gap-2 pb-2">
        <Choice
          label={m["shell.panel.settings"]()}
          value={tab}
          onValueChange={(t) => {
            void onRun(TAB_META[t].command);
          }}
          options={SETTINGS_TABS.map((t) => {
            const { icon: Icon, label } = TAB_META[t];
            return { value: t, label, icon: <Icon className="size-4" /> };
          })}
          className="min-w-0 flex-1"
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {tab === "editor" && (
          <EditorSettingsPage
            vimMode={vimMode}
            onToggleVim={() => void onRun("ui vim")}
            onOpenCommands={() => void onRun("ui settings commands")}
          />
        )}
        {tab === "io" && <IoPanel engine={engine} />}
        {tab === "project" && <ProjectSettingsTab onRun={onRun} />}
      </div>
    </div>
  );
}
