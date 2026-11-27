// ProjectsTable.tsx — the home page's workspaces as cards: each a region with its name, its count, its git pills
// (Pull in place, Save), New project and ⋮, then its project rows. A card folds.
// What reads wrong: a file dropped on any part of a card imports into that workspace, so the drop handlers sit on
// the card itself (`data-workspace` stays for the e2e). See ../../docs/projects.md.

import type { ProjectShape } from "@studio/k7-wasm/types";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  FolderPlus,
} from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import { EXAMPLES_WORKSPACE } from "@studio/web-protocol/projects";
import { type DragEvent, useEffect, useState } from "react";
import {
  CardGitNote,
  CardGitPills,
  CardPullBar,
  useCardGit,
} from "../git/GitBar";
import { lockSentence } from "../git/gitLock";
import { useLocale } from "../i18n";
import { m } from "../paraglide/messages.js";
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

/** One workspace's card: header, pull bar, then its rows. A component so its git state is read once, by the card. */
function WorkspaceCard({
  w,
  p,
  now,
  locale,
  over,
  setOver,
  open,
  onFold,
  targets,
}: {
  w: AccountWorkspace;
  p: Props;
  now: number;
  locale: string;
  over: string | null;
  setOver: (
    name: string | null | ((o: string | null) => string | null),
  ) => void;
  open: boolean;
  onFold: () => void;
  targets: AccountWorkspace[];
}) {
  const git = useCardGit(w.name);
  const locked = !!p.locked?.(w);
  // A merge waiting here refuses every write, so the card says so before a dialog is filled in.
  const lockSaid = locked ? lockSentence() : "";
  const writeBlocked = p.blocked || lockSaid;
  const lit = over === w.name && takesDrops(w, locked);
  const Chevron = open ? ChevronDown : ChevronRight;
  const drop = {
    onDragOver: (e: DragEvent) => {
      if (!carriesFiles(e)) return;
      // Both, or the browser opens the file: navigation is a dropped file's default.
      e.preventDefault();
      if (!takesDrops(w, locked)) {
        e.dataTransfer.dropEffect = "none";
        return;
      }
      e.dataTransfer.dropEffect = "copy";
      setOver(w.name);
    },
    onDragLeave: (e: DragEvent) => {
      // Leaving for a CHILD is not leaving: the card's own dragover would set it straight back, flickering.
      if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
      setOver((o) => (o === w.name ? null : o));
    },
    onDrop: (e: DragEvent) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      setOver(null);
      if (takesDrops(w, locked))
        p.onDropProject(w, Array.from(e.dataTransfer.files));
    },
  };
  return (
    <RowGroup
      role="region"
      aria-label={m["repos.card"]({ name: w.name })}
      data-workspace={w.name}
      data-testid="workspace-card"
      className={lit ? "shadow-(--k7-focus-ring)" : undefined}
      {...drop}
    >
      <div className="flex min-h-[60px] flex-wrap items-center gap-x-2 gap-y-1 px-3.5 py-2">
        <button
          type="button"
          className="grid size-11 shrink-0 cursor-pointer desk:size-8 place-items-center rounded-full text-fg-2 outline-none hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring)"
          aria-expanded={open}
          aria-label={
            open
              ? m["repos.foldWorkspace"]({ name: w.name })
              : m["repos.unfoldWorkspace"]({ name: w.name })
          }
          onClick={onFold}
        >
          <Chevron className="size-4" />
        </button>
        {/* The name folds the card like the chevron beside it, which is the keyboard's way in. */}
        <button
          type="button"
          tabIndex={-1}
          className="group flex min-w-0 flex-1 basis-40 cursor-pointer items-center gap-2.5 rounded-xl text-left outline-none focus-visible:shadow-(--k7-focus-ring)"
          onClick={onFold}
        >
          <Folder className="size-5 shrink-0 text-primary-text" />
          <span className="min-w-0 truncate text-[17px] font-bold">
            {w.name}
          </span>
          <span className="type-secondary shrink-0 text-fg-3 tabular-nums max-sm:hidden">
            {projectsCount(w.projects.length)}
          </span>
        </button>
        <CardGitNote git={git} />
        <CardGitPills git={git} workspace={w.name} demo={isExamples(w)} />
        {!isExamples(w) && (
          <NewProject
            blocked={writeBlocked}
            workspace={w.name}
            onCreate={p.createIn}
          />
        )}
        <WorkspaceMenu
          workspace={w.name}
          projectCount={w.projects.length}
          blocked={p.blocked}
          locked={lockSaid}
          onBrowse={() => p.onOpen(w)}
          onRemove={() => p.onRemoveWorkspace(w)}
          onConnected={p.onChanged}
        />
      </div>
      <CardPullBar git={git} workspace={w.name} />
      {lockSaid && (
        <p role="status" className="type-secondary px-3.5 pb-2 text-warn">
          {lockSaid}
        </p>
      )}
      {open &&
        (!w.projects.length && !isExamples(w) ? (
          // Nothing in it yet: the link opens the WORKSPACE and the editor asks for the first project (FirstProject.tsx).
          <a
            className="type-body flex min-h-14 items-center gap-3 px-3.5 py-2 text-fg-2 outline-none hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring)"
            href={p.workspaceHref(w)}
            title={m["repos.openAndCreate"]({ name: w.name })}
          >
            <FolderPlus className="size-5 shrink-0 text-primary-text" />
            <span>{m["repos.createFirst"]()}</span>
          </a>
        ) : (
          w.projects.map((x) => (
            <ProjectRow
              key={`${w.name}/${x.path}`}
              workspace={w}
              project={x}
              href={p.projectHref(w, x)}
              targets={targets}
              blocked={writeBlocked}
              now={now}
              locale={locale}
              onCopy={(to, folder, onStep) =>
                p.onCopy(w, x, to, folder, onStep)
              }
              onRename={(folder, onStep) => p.onRename(w, x, folder, onStep)}
              onDelete={(onStep) => p.onDelete(w, x, onStep)}
            />
          ))
        ))}
    </RowGroup>
  );
}

interface Props {
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
  /** Whether a workspace is read-only here: its cards take no drop and light up for none. */
  locked?: (ws: AccountWorkspace) => boolean;
}

export function ProjectsTable(p: Props) {
  const now = useMinuteClock();
  const { locale } = useLocale();
  // Which card the pointer drags over, so exactly one workspace lights up.
  const [over, setOver] = useState<string | null>(null);
  // Folded by hand: whether Examples is listed at all is the page's ⋮ (ProjectsMenu.tsx).
  const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set());
  return (
    <div className="flex min-w-0 flex-col gap-4">
      {p.workspaces.map((w) => (
        <WorkspaceCard
          key={w.name}
          w={w}
          p={p}
          now={now}
          locale={locale}
          over={over}
          setOver={setOver}
          open={!folded.has(w.name)}
          onFold={() =>
            setFolded((s) => {
              const next = new Set(s);
              if (!next.delete(w.name)) next.add(w.name);
              return next;
            })
          }
          targets={targetsFor(p.workspaces, w)}
        />
      ))}
    </div>
  );
}
