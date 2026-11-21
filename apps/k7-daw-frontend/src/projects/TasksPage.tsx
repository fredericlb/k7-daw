// TasksPage.tsx — `/tasks`, a page of the home: every workspace's TODOS.md in one list, grouped by
// workspace, each task linking to the project its anchor names. READ-ONLY on purpose: a task is
// ticked inside its workspace, where the `task` commands run. See ../../docs/tasks.md.

import { Button } from "@studio/ui/button";
import { EmptyState } from "@studio/ui/empty-state";
import { ErrorCard } from "@studio/ui/error-card";
import {
  ArrowRight,
  CheckCircle2,
  Circle,
  ListChecks,
  RefreshCw,
} from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { Switch } from "@studio/ui/switch";
import { useCallback, useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { useAccountFiles } from "../settings/useAccountFiles";
import { accountFiles, projectHref, workspaceHref } from "./api";
import { HomeShell } from "./HomeFrame";
import { PageFrame } from "./PageFrame";
import type { AccountFiles } from "./workspaceOps";
import {
  anchorProject,
  readAllTasks,
  type WorkspaceTasks,
} from "./workspaceTasks";

export function TasksPage() {
  // `undefined` while asked: "none answers" is drawn only once that is the answer.
  const fileService = useAccountFiles();

  const [lists, setLists] = useState<WorkspaceTasks[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showDone, setShowDone] = useState(false);

  // Read on arrival and on Refresh, not polled: N workspaces is N reads, and the list is visited rather than watched.
  const load = useCallback(async (f: AccountFiles) => {
    setLists(null);
    try {
      setLists(await readAllTasks(f, await accountFiles.workspaces(f)));
      setError(null);
    } catch (e) {
      setError((e as Error).message);
      setLists([]);
    }
  }, []);
  useEffect(() => {
    if (fileService) void load(fileService);
  }, [fileService, load]);

  const open = (lists ?? []).reduce(
    (n, l) => n + l.tasks.filter((t) => !t.done).length,
    0,
  );
  // A workspace with nothing to show under the current filter is left out rather than drawn empty.
  const shown = (lists ?? [])
    .map((l) => ({
      ...l,
      open: l.tasks.filter((t) => !t.done).length,
      tasks: showDone ? l.tasks : l.tasks.filter((t) => !t.done),
    }))
    .filter((l) => l.tasks.length > 0 || l.error);

  return (
    <HomeShell>
      <PageFrame
        title={m["home.tasks"]()}
        summary={
          fileService && lists !== null
            ? m["tasks.openCount"]({ n: open })
            : undefined
        }
        // The page's own controls sit with its heading, as on Projects, not in the sidebar's foot.
        actions={
          fileService ? (
            <>
              <label
                htmlFor="tasks-show-done"
                className="type-secondary flex cursor-pointer items-center gap-2 text-fg-2"
              >
                <Switch
                  id="tasks-show-done"
                  size="sm"
                  checked={showDone}
                  onCheckedChange={setShowDone}
                />
                {m["tasks.showDone"]()}
              </label>
              <Button
                size="icon"
                variant="glass"
                disabled={lists === null}
                onClick={() => void load(fileService)}
                title={m["tasks.refresh"]()}
                aria-label={m["tasks.refresh"]()}
              >
                <RefreshCw className="size-5" />
              </Button>
            </>
          ) : undefined
        }
      >
        {fileService === null ? (
          <ErrorCard title={m["repos.noFilesTitle"]()} />
        ) : fileService === undefined || lists === null ? (
          <div className="type-secondary flex items-center gap-2 text-fg-2">
            <Spinner />
            {m["tasks.reading"]()}
          </div>
        ) : (
          <>
            {error && <ErrorCard title={error} />}
            {shown.length === 0 && !error && (
              // No act on the spot: a task is written inside its workspace, where the `task` commands run.
              <EmptyState glyph={<ListChecks />} title={m["tasks.noneTitle"]()}>
                {m["tasks.none"]()}
              </EmptyState>
            )}
            {shown.map(
              ({ workspace: ws, tasks, open: wsOpen, error: wsError }) => (
                <section
                  key={ws.name}
                  aria-label={m["tasks.of"]({ workspace: ws.name })}
                  className="flex flex-col gap-2"
                >
                  <h2 className="flex items-center justify-between gap-2 px-1">
                    <span className="flex min-w-0 items-baseline gap-2">
                      <span className="type-caps truncate text-fg-3">
                        {ws.name}
                      </span>
                      <span className="type-status shrink-0 text-fg-3 tabular-nums">
                        {m["tasks.openCount"]({ n: wsOpen })}
                      </span>
                    </span>
                    {/* An ANCHOR, not a router link: a project is the router's basename, so opening one is a whole-page navigation. */}
                    <a
                      href={workspaceHref(ws)}
                      aria-label={m["tasks.openNamed"]({ name: ws.name })}
                      className="type-secondary flex shrink-0 items-center gap-1 font-bold text-primary-text hover:underline"
                    >
                      {m["tasks.openWorkspace"]()}
                      <ArrowRight className="size-4" />
                    </a>
                  </h2>
                  {wsError && (
                    <ErrorCard title={m["tasks.readFailed"]()}>
                      <code>{wsError}</code>
                    </ErrorCard>
                  )}
                  {tasks.length > 0 && (
                    <RowGroup>
                      {tasks.map((t) => {
                        const p = anchorProject(ws, t.at);
                        return (
                          <div
                            key={t.line}
                            className="type-body flex min-h-11 items-start gap-3 px-3.5 py-2.5"
                          >
                            {t.done ? (
                              <CheckCircle2
                                role="img"
                                aria-label={m["tasks.statusDone"]()}
                                className="mt-0.5 size-5 shrink-0 text-ok"
                              />
                            ) : (
                              <Circle
                                role="img"
                                aria-label={m["tasks.statusOpen"]()}
                                className="mt-0.5 size-5 shrink-0 text-fg-3"
                              />
                            )}
                            <span
                              className={
                                t.done
                                  ? "min-w-0 flex-1 text-fg-3 line-through"
                                  : "min-w-0 flex-1"
                              }
                            >
                              {t.text}
                            </span>
                            {t.at &&
                              (p ? (
                                <a
                                  href={projectHref(ws, p)}
                                  className="type-mono-value shrink-0 text-primary-text hover:underline"
                                  title={m["tasks.openProject"]({
                                    path: p.path,
                                  })}
                                >
                                  {t.at.project} @ {t.at.timecode}
                                </a>
                              ) : (
                                // The anchor names a project the workspace no longer holds: said, not linked.
                                <span
                                  className="type-mono-value shrink-0 text-fg-3"
                                  title={m["tasks.orphan"]({
                                    project: t.at.project,
                                  })}
                                >
                                  {t.at.project} @ {t.at.timecode}
                                </span>
                              ))}
                          </div>
                        );
                      })}
                    </RowGroup>
                  )}
                </section>
              ),
            )}
          </>
        )}
      </PageFrame>
    </HomeShell>
  );
}
