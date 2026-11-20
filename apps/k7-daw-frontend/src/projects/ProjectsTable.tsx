// ProjectsTable.tsx — the home page's workspaces and projects in the songs' table (BandTable): each
// workspace a band, with its sync state, its count, its New project and its ⋮; its projects under it;
// A band folds.
// What reads wrong: every row of a workspace carries `data-workspace`, the table having no element
// that groups them, and a file dropped on any of them imports into that workspace. See ../../docs/projects.md.

import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  FolderPlus,
} from "@studio/ui/icons";
import { TableCell, TableRow } from "@studio/ui/table";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import { EXAMPLES_WORKSPACE } from "@studio/web-protocol/projects";
import { type DragEvent, useEffect, useState } from "react";
import { WorkspaceGitRow } from "../git/GitBar";
import { useLocale } from "../i18n";
import { m } from "../paraglide/messages.js";
import {
  BAND_ROW,
  type BandColumn,
  type BandItem,
  BandSearch,
  BandTable,
  bandCells,
} from "./BandTable";
import { NewProject } from "./NewProject";
import { ProjectRow } from "./ProjectRow";
import { WorkspaceMenu } from "./WorkspaceMenu";

/** The bundled demos' workspace: listed last, folded, and taking no new project. See ../../docs/projects.md. */
export const isExamples = (w: AccountWorkspace): boolean =>
  w.name === EXAMPLES_WORKSPACE;

/** "1 project" / "4 projects", the one place the count's plural is chosen. */
export const projectsCount = (n: number): string =>
  (n === 1 ? m["repos.projects.one"] : m["repos.projects.other"])({ n });

/** Where a copy of a project may land: every workspace but its own, nor one that IS a project, where a copy would never show. */
const targetsFor = (
  all: AccountWorkspace[],
  from: AccountWorkspace,
): AccountWorkspace[] =>
  all.filter(
    (w) => w.name !== from.name && !w.isSingleProject && !isExamples(w),
  );

/** A clock for "edited … ago", ticking once a minute: the list re-renders only when it changes otherwise. */
function useMinuteClock(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

/** Whether a workspace row takes a dropped file: not the demos', nor one this device only reads (docs/sharing.md). */
export const takesDrops = (w: AccountWorkspace, locked: boolean): boolean =>
  !isExamples(w) && !locked;

/** Whether a drag carries files at all: a dragover knows kinds, never names, so the DROP is where it is judged. */
function carriesFiles(e: DragEvent): boolean {
  const items = Array.from(e.dataTransfer.items ?? []);
  return items.length
    ? items.some((i) => i.kind === "file")
    : e.dataTransfer.files.length > 0;
}

/** Whether a search keeps this project: every word in the workspace's name, the project's path or its tags. */
const matches = (w: AccountWorkspace, p: RepoProject, words: string[]) => {
  const hay = `${w.name} ${p.path} ${(p.tags ?? []).join(" ")}`.toLowerCase();
  return words.every((x) => hay.includes(x));
};

export function ProjectsTable(p: {
  /** In the order the page lists them, Examples last. */
  workspaces: AccountWorkspace[];
  blocked: string;
  onOpen: (ws: AccountWorkspace) => void;
  onDropProject: (ws: AccountWorkspace, files: File[]) => void;
  createIn: (
    name: string,
    folder: string,
    onStep: (s: string) => void,
    shape: ProjectShape,
  ) => Promise<void>;
  onDelete: (
    ws: AccountWorkspace,
    project: RepoProject,
    onStep: (s: string) => void,
  ) => Promise<void>;
  onRename: (
    ws: AccountWorkspace,
    project: RepoProject,
    folder: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  onCopy: (
    ws: AccountWorkspace,
    project: RepoProject,
    to: AccountWorkspace,
    folder: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  projectHref: (ws: AccountWorkspace, project: RepoProject) => string;
  workspaceHref: (ws: AccountWorkspace) => string;
  onRemoveWorkspace: (ws: AccountWorkspace) => Promise<void>;
  onChanged: () => void;
  /** Whether a workspace is read-only here: its rows take no drop and light up for none. */
  locked?: (ws: AccountWorkspace) => boolean;
}) {
  const now = useMinuteClock();
  const { locale } = useLocale();
  const [query, setQuery] = useState("");
  // Which row the pointer drags over, so exactly one workspace lights up.
  const [over, setOver] = useState<string | null>(null);
  // Folded by hand: whether Examples is listed at all is the page's ⋮ (ProjectsMenu.tsx).
  const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set());
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const searching = words.length > 0;

  const columns: BandColumn[] = [
    { head: m["repos.colProject"]() },
    { head: m["repos.colFolder"](), className: "hidden md:table-cell" },
    { head: m["repos.colEdited"](), className: "hidden w-36 sm:table-cell" },
    { className: "w-28 sm:w-48" },
  ];

  const dropOn = (w: AccountWorkspace) => ({
    className:
      over === w.name && takesDrops(w, !!p.locked?.(w)) ? "bg-primary/10" : "",
    onDragOver: (e: DragEvent) => {
      if (!carriesFiles(e)) return;
      // Both, or the browser opens the file: navigation is a dropped file's default.
      e.preventDefault();
      if (!takesDrops(w, !!p.locked?.(w))) {
        e.dataTransfer.dropEffect = "none";
        return;
      }
      e.dataTransfer.dropEffect = "copy";
      setOver(w.name);
    },
    onDragLeave: (e: DragEvent) => {
      // Leaving for a CHILD is not leaving: the row's own dragover would set it straight back, flickering.
      if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
      setOver((o) => (o === w.name ? null : o));
    },
    onDrop: (e: DragEvent) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      setOver(null);
      if (takesDrops(w, !!p.locked?.(w)))
        p.onDropProject(w, Array.from(e.dataTransfer.files));
    },
  });

  const band = (w: AccountWorkspace, shown: number) => {
    const open = searching || !folded.has(w.name);
    const Chevron = open ? ChevronDown : ChevronRight;
    const { className: lit, ...drop } = dropOn(w);
    // The git badge wraps below the name when the column is narrow, rather than squeezing beside it.
    const label = (
      <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-foreground">
        <button
          type="button"
          className="shrink-0 rounded p-0.5 text-muted-foreground hover:bg-accent/60 hover:text-foreground"
          aria-expanded={open}
          aria-label={
            open
              ? m["repos.foldWorkspace"]({ name: w.name })
              : m["repos.unfoldWorkspace"]({ name: w.name })
          }
          onClick={() =>
            setFolded((s) => {
              const next = new Set(s);
              if (!next.delete(w.name)) next.add(w.name);
              return next;
            })
          }
        >
          <Chevron className="h-4 w-4" />
        </button>
        {/* The workspace opens on its name, as its card did. */}
        <button
          type="button"
          className="group flex min-w-0 items-center gap-2 rounded px-1 text-left hover:bg-accent/60"
          onClick={() => p.onOpen(w)}
        >
          <Folder className="h-4 w-4 shrink-0 text-muted-foreground group-hover:text-foreground" />
          <span className="min-w-16 truncate font-semibold">{w.name}</span>
          {/* A count is read, not acted on: plain muted text. */}
          <span className="hidden shrink-0 font-normal text-muted-foreground text-xs tabular-nums sm:inline">
            {projectsCount(searching ? shown : w.projects.length)}
          </span>
        </button>
        <WorkspaceGitRow workspace={w.name} />
      </span>
    );
    const acts = (
      <span className="inline-flex items-center justify-end gap-1">
        {!isExamples(w) && (
          <NewProject
            blocked={p.blocked}
            workspaces={[w.name]}
            onCreate={p.createIn}
          />
        )}
        <WorkspaceMenu
          workspace={w.name}
          projectCount={w.projects.length}
          blocked={p.blocked}
          onRemove={() => p.onRemoveWorkspace(w)}
          onConnected={p.onChanged}
        />
      </span>
    );
    return { open, lit, drop, label, acts };
  };

  const items: BandItem[] = [];
  for (const w of p.workspaces) {
    // A workspace named by the search keeps all of itself, an empty one included; else only its matching projects.
    const named = words.every((x) => w.name.toLowerCase().includes(x));
    const kept =
      searching && !named
        ? w.projects.filter((x) => matches(w, x, words))
        : w.projects;
    if (searching && !named && !kept.length) continue;
    const b = band(w, kept.length);
    items.push({
      key: `ws:${w.name}`,
      draw: (mm) => (
        <TableRow
          key={`ws:${w.name}`}
          {...mm}
          {...b.drop}
          data-workspace={w.name}
          data-testid="workspace-card"
          className={`${BAND_ROW} ${b.lit}`}
        >
          {bandCells(columns, b.label, "pt-5 text-sm", b.acts)}
        </TableRow>
      ),
    });
    if (!b.open) continue;
    // Nothing in it yet: the link opens the WORKSPACE and the editor asks for the first project (FirstProject.tsx).
    if (!w.projects.length && !isExamples(w)) {
      items.push({
        key: `first:${w.name}`,
        draw: (mm) => (
          <TableRow key={`first:${w.name}`} {...mm} data-workspace={w.name}>
            <TableCell className="pl-8">
              <a
                className="flex w-fit items-center gap-2 text-sm hover:underline"
                href={p.workspaceHref(w)}
                title={m["repos.openAndCreate"]({ name: w.name })}
              >
                <FolderPlus className="h-4 w-4 shrink-0 text-muted-foreground" />
                <span>{m["repos.createFirst"]()}</span>
              </a>
            </TableCell>
            <TableCell className="hidden md:table-cell" />
            <TableCell className="hidden sm:table-cell" />
            <TableCell />
          </TableRow>
        ),
      });
      continue;
    }
    const targets = targetsFor(p.workspaces, w);
    for (const x of kept)
      items.push({
        key: `p:${w.name}/${x.path}`,
        draw: (mm) => (
          <ProjectRow
            key={`p:${w.name}/${x.path}`}
            measured={mm}
            workspace={w}
            project={x}
            href={p.projectHref(w, x)}
            targets={targets}
            blocked={p.blocked}
            now={now}
            locale={locale}
            drop={dropOn(w)}
            onCopy={(to, folder, onStep) => p.onCopy(w, x, to, folder, onStep)}
            onRename={(folder, onStep) => p.onRename(w, x, folder, onStep)}
            onDelete={(onStep) => p.onDelete(w, x, onStep)}
          />
        ),
      });
  }

  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <BandSearch
          value={query}
          onChange={setQuery}
          label={m["repos.search"]()}
          clearLabel={m["repos.clearSearch"]()}
        />
      </div>
      <BandTable columns={columns} items={items} />
      {searching && !items.length ? (
        <p className="py-6 text-center text-muted-foreground text-sm">
          {m["repos.noMatch"]({ query: query.trim() })}
        </p>
      ) : null}
    </div>
  );
}
