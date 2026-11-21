// EditorToolbar.tsx — the editor pane's top bar: the breadcrumb, a sequence's three views in the
// popin, the module builder's door, a clip's Process menu, and the save state in place of a Save
// button — and the bar that says the open file moved on disk under an edit. See ../../docs/shell.md.

import { docPattern } from "@studio/song-schema/docExt";
import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  FileText,
  LayoutGrid,
  Loader2,
  Piano,
  TriangleAlert,
  Wand2,
} from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { AutosaveStatus } from "@studio/ui-editor/autosave";
import { m } from "../paraglide/messages.js";
import {
  isClipFile,
  isModuleFile,
  moduleFileOf,
  projectDir,
} from "../projectPaths";
import { needsAttention, saveLabel } from "../saveLabel";
import type { FileContent, FileNode } from "../types";
import { Breadcrumb } from "./Breadcrumb";
import { ChainOpsList } from "./ChainOpsList";

const UI_FILE = docPattern("ui");

/** The popin's two views of a sequence: the file, and its notes. */
export type SeqTab = "text" | "roll";

// The two tabs, in the order they read: the file, then its notes.
const SEQ_TABS = [
  {
    id: "text",
    label: () => m["editor.tabText"](),
    icon: FileText,
    title: () => m["editor.tabTextHint"](),
  },
  {
    id: "roll",
    label: () => m["editor.tabRoll"](),
    icon: Piano,
    title: () => m["editor.tabRollHint"](),
  },
] as const;

export function EditorToolbar({
  tree,
  file,
  activeProject,
  projectName,
  onOpen,
  onOpenProject,
  seqTabs,
  tab,
  onTab,
  onOpenBuilder,
  processing,
  opSearch,
  onOpSearch,
  clipOps,
  onProcess,
  autosave,
}: {
  tree: FileNode[];
  file: FileContent | null;
  activeProject: string | null;
  projectName: string | null;
  onOpen: (path: string) => void;
  onOpenProject: (path: string) => void;
  /** Whether the sequence tabs are offered: a sequence, in the popin. */
  seqTabs: boolean;
  tab: SeqTab;
  onTab: (tab: SeqTab) => void;
  onOpenBuilder: (path: string, pane: "module" | "panel") => void;
  processing: boolean;
  opSearch: string;
  onOpSearch: (q: string) => void;
  clipOps: string[];
  onProcess: (chain: string) => void;
  autosave: { status: AutosaveStatus; reason: string | null };
}) {
  return (
    <div className="flex items-center gap-2 border-b border-border px-3 py-1.5">
      <Breadcrumb
        tree={tree}
        path={file?.path ?? null}
        // Where `tree` is rooted: paths stay workspace-relative, so the breadcrumb needs BOTH to tell the two spaces apart.
        root={projectDir(activeProject)}
        // In a workspace the folder name IS the project; alone, only the song knows what it is called.
        rootLabel={
          projectDir(activeProject).split("/").pop() || projectName || "project"
        }
        activeProject={activeProject}
        onOpen={onOpen}
        onOpenProject={onOpenProject}
      />
      <div className="flex-1" />
      {/* A sequence in the POPIN gets its two views as tabs, the text and the roll as peers (docs/shell.md). */}
      {seqTabs && (
        <div
          role="tablist"
          aria-label={m["seq.views"]()}
          className="glass flex h-10 items-center gap-0.5 rounded-full p-[3px]"
        >
          {SEQ_TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              title={t.title()}
              className={cn(
                "flex h-8 items-center gap-1 rounded-full px-3 font-bold text-[13px] transition-colors",
                tab === t.id
                  ? "bg-on-fill text-lavender"
                  : "text-foreground/80 hover:bg-on-fill/60",
              )}
              onClick={() => onTab(t.id)}
            >
              <t.icon className="h-3.5 w-3.5" />
              {t.label()}
            </button>
          ))}
        </div>
      )}
      {/* A BUTTON and not a redirected click: opening `module.k7x` must still open its text, the path a shared module forks through. */}
      {file && isModuleFile(file.path) && (
        <Button
          size="sm"
          variant="secondary"
          title={m["editor.openInBuilder"]()}
          onClick={() =>
            onOpenBuilder(
              moduleFileOf(file.path),
              UI_FILE.test(file.path) ? "panel" : "module",
            )
          }
        >
          <LayoutGrid className="mr-1 h-4 w-4" />
          Builder
        </Button>
      )}
      {/* The waveform panel's "operations" menu on the clip's TEXT (../clipOps.ts); the reload keeps the buffer off the repointed `source:`. */}
      {file && isClipFile(file.path) && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              size="sm"
              variant="secondary"
              disabled={processing}
              title={m["editor.runChain"]()}
            >
              {processing ? (
                <Loader2 className="mr-1 h-4 w-4 animate-spin" />
              ) : (
                <Wand2 className="mr-1 h-4 w-4" />
              )}
              Process
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-64 p-1">
            <ChainOpsList
              search={opSearch}
              onSearch={onOpSearch}
              ops={clipOps}
              onPick={onProcess}
            />
          </DropdownMenuContent>
        </DropdownMenu>
      )}
      {/* In place of a Save button: silent while the file is saved, amber with the reason for as long as it is not. */}
      <span
        className={cn(
          "flex min-w-0 shrink items-center gap-1 text-xs",
          needsAttention(autosave.status)
            ? "text-amber-500"
            : "text-muted-foreground",
        )}
        title={autosave.reason ?? undefined}
      >
        {autosave.status === "saving" && (
          <Loader2 className="h-3 w-3 shrink-0 animate-spin" />
        )}
        <span className="truncate">
          {saveLabel(autosave.status, autosave.reason)}
        </span>
      </span>
    </div>
  );
}

/** The open file changed on disk under an edit in progress: both versions are real work, so this states the choice and waits. */
export function DiskConflictBar({
  onKeep,
  onTake,
}: {
  onKeep: () => void;
  onTake: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-amber-500/40 bg-amber-500/10 px-3 py-1.5 text-xs">
      <TriangleAlert className="h-4 w-4 shrink-0 text-amber-500" />
      <span className="min-w-0 flex-1">{m["disk.changed"]()}</span>
      {/* Each button says what it THROWS AWAY: both versions are someone's work. */}
      <Button size="sm" variant="secondary" onClick={onKeep}>
        {m["disk.keepMine"]()}
      </Button>
      <Button size="sm" onClick={onTake}>
        {m["disk.takeTheirs"]()}
      </Button>
    </div>
  );
}
