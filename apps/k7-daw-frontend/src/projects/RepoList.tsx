// RepoList.tsx — the account's workspaces and their projects: the Home top bar (title, New, More), then the
// workspace cards (ProjectsTable.tsx), "On your other devices" at their foot, or what stands in for them when there is
// nothing to list. Presentational — the section holds the async and this file draws. See ../../docs/projects.md.

import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { EmptyState } from "@studio/ui/empty-state";
import {
  Folder,
  GitBranch,
  HardDrive,
  Loader2,
  Plus,
  ServerOff,
} from "@studio/ui/icons";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import { byPeople, byPeoplePath } from "@studio/web-protocol/tree";
import { useState } from "react";
import { Link } from "react-router-dom";
import { CloneDialog } from "../git/CloneWorkspace";
import { useGitTransport } from "../git/GitContext";
import { lockSentence } from "../git/gitLock";
import { OtherDevices } from "../git/OtherDevices";
import { useMergingWorkspaces } from "../git/useMergingWorkspaces";
import { parts } from "../i18nParts";
import { m } from "../paraglide/messages.js";
import { SETTINGS_ROUTE } from "./api";
import { NewWorkspace } from "./NewWorkspace";
import { PAGE_COLUMN, PageHeader } from "./PageFrame";
import { ProjectsMenu, useShowExamples } from "./ProjectsMenu";
import { isExamples, ProjectsTable, projectsCount } from "./ProjectsTable";

/** The account's workspaces in the order the page lists them: for people, Examples last, each one's projects by path (../../docs/listOrder.md). */
export const pageOrder = (all: AccountWorkspace[]): AccountWorkspace[] =>
  all
    .map((w) => ({
      ...w,
      projects: [...w.projects].sort((a, b) => byPeoplePath(a.path, b.path)),
    }))
    .sort(
      (a, b) =>
        Number(isExamples(a)) - Number(isExamples(b)) ||
        byPeople(a.name, b.name),
    );

/** …and the same for workspaces, in the heading's summary. */
const workspacesCount = (n: number): string =>
  (n === 1 ? m["repos.workspaces.one"] : m["repos.workspaces.other"])({ n });

export function RepoList({
  workspaces,
  busy,
  error,
  notice = null,
  hasFileService,
  onCreateWorkspace,
  onOpen,
  blocked,
  blockedWorkspace,
  onDropProject,
  onCreate,
  onDelete,
  onRemoveWorkspace,
  onCopy,
  onRename,
  projectHref,
  workspaceHref,
  onChanged,
  lookingForFiles = false,
}: {
  workspaces: AccountWorkspace[];
  busy: boolean;
  error: string | null;
  /** A warning in plain words, drawn amber rather than as an error. */
  notice?: string | null;
  hasFileService: boolean;
  /** Still asking for the file service: the list comes once it answers, so a spinner, not a refusal. */
  lookingForFiles?: boolean;
  /** A workspace of one's own, made on the server with nothing in it. */
  onCreateWorkspace: (
    workspace: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  onOpen: (ws: AccountWorkspace) => void;
  /** Why create and delete are impossible, or "" — both go through the file service, which is also what READS this list (../../docs/projects.md). */
  blocked: string;
  /** What stops a workspace being MADE, on top of `blocked` — the account's library, which everything in one resolves through. */
  blockedWorkspace: string;
  /** Files dropped ON a row: everything dropped, since which one is importable — and what to say
   *  when none is — belongs with the state that can report it (apps/k7-daw-frontend/packages/translator/docs/import.md). */
  onDropProject: (ws: AccountWorkspace, files: File[]) => void;
  onCreate: (
    ws: AccountWorkspace,
    folder: string,
    onStep: (s: string) => void,
    /** `arrangement`, `mix` or `video`. See NewProject. */
    shape: ProjectShape,
  ) => Promise<void>;
  onDelete: (
    ws: AccountWorkspace,
    project: RepoProject,
    onStep: (s: string) => void,
  ) => Promise<void>;
  /** Rename a project's folder in place. */
  onRename: (
    ws: AccountWorkspace,
    project: RepoProject,
    folder: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  /** Copy a project into ANOTHER workspace. */
  onCopy: (
    ws: AccountWorkspace,
    project: RepoProject,
    to: AccountWorkspace,
    folder: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  /** A project's address in ITS app — an address and not an action: the row is a link, opening in a new tab. */
  projectHref: (ws: AccountWorkspace, project: RepoProject) => string;
  /** …and where the WORKSPACE opens, for a row that has no project to link to yet. */
  workspaceHref: (ws: AccountWorkspace) => string;
  /** Delete a whole workspace — the folder and everything in it. */
  onRemoveWorkspace: (ws: AccountWorkspace) => Promise<void>;
  /** After a revocation, so the page re-reads without waiting for the poll. */
  onChanged: () => void;
}) {
  // Examples is listed when the ⋮ says so, by default only while nothing of one's own is.
  const [showExamples, setShowExamples] = useShowExamples(
    workspaces.some((w) => !isExamples(w)),
  );
  // A workspace waiting for a merge's choices takes no write from the home page (docs/git.md).
  const merges = useMergingWorkspaces(
    useGitTransport(),
    workspaces.map((w) => w.name),
  );
  const refuse = (name: string): Promise<never> | null =>
    merges.merging(name) ? Promise.reject(new Error(lockSentence())) : null;
  const listed = workspaces.filter((w) => showExamples || !isExamples(w));
  const projectTotal = listed.reduce((n, w) => n + w.projects.length, 0);
  const ordered = pageOrder(listed);
  const createIn = (
    name: string,
    folder: string,
    onStep: (s: string) => void,
    shape: ProjectShape,
  ) => {
    const w = workspaces.find((x) => x.name === name);
    if (!w) return Promise.reject(new Error(name));
    return refuse(w.name) ?? onCreate(w, folder, onStep, shape);
  };
  // The New menu's two dialogs live here: a menu unmounts its items on select, and a dialog inside would go with them.
  const [making, setMaking] = useState(false);
  const [cloning, setCloning] = useState(false);
  const newBlocked = blockedWorkspace;
  return (
    <div className={PAGE_COLUMN}>
      <PageHeader
        title={m["home.projects"]()}
        summary={
          listed.length > 0
            ? `${projectsCount(projectTotal)} · ${workspacesCount(listed.length)}`
            : undefined
        }
        actions={
          <>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="glass"
                  size="icon"
                  title={m["home.newMenu"]()}
                  aria-label={m["home.newMenu"]()}
                >
                  <Plus className="size-5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" aria-label={m["home.new"]()}>
                <DropdownMenuItem
                  disabled={Boolean(newBlocked)}
                  title={newBlocked || m["ws.newHint"]()}
                  onSelect={() => setMaking(true)}
                >
                  <HardDrive />
                  {m["ws.new"]()}
                </DropdownMenuItem>
                <DropdownMenuItem
                  disabled={Boolean(blocked)}
                  title={blocked || m["git.cloneHint"]()}
                  onSelect={() => setCloning(true)}
                >
                  <GitBranch />
                  {m["git.clone"]()}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <ProjectsMenu
              showExamples={showExamples}
              onShowExamples={setShowExamples}
            />
          </>
        }
      />
      <NewWorkspace
        blocked={blockedWorkspace}
        onCreate={onCreateWorkspace}
        shown={making}
        onShown={setMaking}
      />
      <CloneDialog
        open={cloning}
        onClose={() => setCloning(false)}
        onCloned={onChanged}
      />

      {error && (
        <code className="max-w-full overflow-x-auto rounded-xl bg-(--k7-error-fill) px-3 py-2 text-error text-xs">
          {error}
        </code>
      )}
      {notice && (
        <p className="rounded-xl border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3 py-2 text-warn text-xs">
          {notice}
        </p>
      )}

      {lookingForFiles ? (
        // The file service boots after the page on a phone: its first silence is not a refusal (docs/projects.md).
        <div className="type-secondary flex items-center gap-2 text-fg-2">
          <Loader2 className="size-4 animate-spin" /> {m["common.loading"]()}
        </div>
      ) : !hasFileService ? (
        // With no file service there is no list, and an empty one would read as a new account.
        <EmptyState glyph={<ServerOff />} title={m["repos.noFilesTitle"]()}>
          {parts((h) => m["repos.noFilesAside"](h), {
            settings: (
              <Link to={SETTINGS_ROUTE} className="underline">
                {m["repos.settingsWord"]()}
              </Link>
            ),
          })}
        </EmptyState>
      ) : workspaces.length === 0 && busy ? (
        <div className="type-secondary flex items-center gap-2 text-fg-2">
          <Loader2 className="size-4 animate-spin" /> {m["common.loading"]()}
        </div>
      ) : workspaces.length === 0 ? (
        // The act itself, on the spot: the menu has the same item, but an empty page is read from its middle.
        <EmptyState
          glyph={<Folder />}
          title={m["repos.emptyTitle"]()}
          actions={
            <>
              <NewWorkspace
                primary
                blocked={blockedWorkspace}
                onCreate={onCreateWorkspace}
              />
              <Button
                variant="glass"
                disabled={Boolean(blocked)}
                title={blocked || m["git.cloneHint"]()}
                onClick={() => setCloning(true)}
              >
                <GitBranch />
                {m["git.clone"]()}
              </Button>
            </>
          }
        >
          {m["repos.empty"]()}
        </EmptyState>
      ) : (
        <ProjectsTable
          workspaces={ordered}
          blocked={blocked}
          onOpen={onOpen}
          onDropProject={onDropProject}
          locked={(w) => merges.merging(w.name)}
          createIn={createIn}
          onDelete={(w, p, step) => refuse(w.name) ?? onDelete(w, p, step)}
          onRename={(w, p, f, step) =>
            refuse(w.name) ?? onRename(w, p, f, step)
          }
          onCopy={(w, p, to, f, step) =>
            refuse(to.name) ?? onCopy(w, p, to, f, step)
          }
          projectHref={projectHref}
          workspaceHref={workspaceHref}
          onRemoveWorkspace={onRemoveWorkspace}
          onChanged={onChanged}
        />
      )}

      {/* The workspaces added on the person's other devices, which git can bring here. */}
      {hasFileService && (
        <OtherDevices
          local={workspaces.map((w) => w.name)}
          blocked={blocked}
          onChanged={onChanged}
        />
      )}
    </div>
  );
}
